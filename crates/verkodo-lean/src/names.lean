-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Elaborates one generated file like `lean` does and prints its messages the same way; in addition writes, as JSON
-- lines, every identifier Lean resolved to a constant: byte range in the file, full name, type, doc string, module,
-- and where it is declared – and every tactic written in the file with the goals before and after it.
-- verkodo writes this file next to the generated ones and runs it with `lake env lean --run`.
-- Usage: lean --run verkodo_names.lean <file.lean> <names.jsonl>
import Lean

open Lean Elab

/-- One resolved identifier as a JSON line. -/
def entry (ctx : ContextInfo) (ti : TermInfo) (name : Name) (r : Syntax.Range) : IO Json := do
  let env := ctx.env
  let some info := env.find? name | return Json.null
  let ty ← ctx.runMetaM ti.lctx (do return toString (← Meta.ppExpr info.type))
  let doc ← findDocString? env name
  let module := (env.getModuleIdxFor? name).bind (env.header.moduleNames[·]?)
  -- Where it is declared: the source file (through `LEAN_SRC_PATH`, which `lake env` sets) and the position of its
  -- name, the line from 1 and the column in UTF-16 code units as editors count them.
  let ranges ← ctx.runMetaM ti.lctx (findDeclarationRanges? name)
  let file ← match module with
    | some m => do
      match ← (← getSrcSearchPath).findWithExt "lean" m with
      | some f => pure (some (← IO.FS.realPath f).toString)
      | none => pure none
    | none => pure none
  let decl := match file, ranges with
    | some f, some r => Json.mkObj [("file", f), ("line", r.selectionRange.pos.line),
        ("character", r.selectionRange.charUtf16)]
    | _, _ => Json.null
  return Json.mkObj [
    ("start", r.start.byteIdx), ("end", r.stop.byteIdx), ("name", name.toString), ("type", ty),
    ("doc", match doc with | some d => Json.str d | none => Json.null),
    ("module", match module with | some m => Json.str m.toString | none => Json.null),
    ("decl", decl)]

/-- Goals as Lean prints them (`ppGoal`), in the metavariable context they belong to. -/
def goals (ctx : ContextInfo) (mctx : MetavarContext) (gs : List MVarId) : IO Json := do
  let ctx := { ctx with mctx := mctx }
  let printed ← gs.toArray.mapM fun g => do
    let s ← ctx.runMetaM {} (do return toString (← Meta.ppGoal g))
    return Json.str s
  return Json.arr printed

unsafe def main (args : List String) : IO UInt32 := do
  let [path, out] := args | IO.eprintln "usage: lean --run verkodo_names.lean <file.lean> <names.jsonl>"; return 2
  let input ← IO.FS.readFile path
  initSearchPath (← findSysroot)
  enableInitializersExecution
  let inputCtx := Parser.mkInputContext input path
  let (header, parserState, messages) ← Parser.parseHeader inputCtx
  let (env, messages) ← processHeader header {} messages inputCtx (mainModule := `Generated)
  let commandState := { Command.mkState env messages {} with infoState.enabled := true }
  let s ← IO.processCommands inputCtx parserState commandState
  let mut errors := false
  for msg in s.commandState.messages.toList do
    IO.print (← msg.toString)
    errors := errors || msg.severity == .error
  let mut lines : Array String := #[]
  for tree in s.commandState.infoState.trees do
    let found := tree.foldInfo (init := #[]) fun ctx info acc =>
      match info with
      | .ofTermInfo ti =>
        match ti.expr.consumeMData, info.range? with
        -- Only identifiers written in the file: notation such as `⟨…⟩` elaborates to synthetic ones.
        | .const name _, some r =>
          match ti.stx, ti.stx.getHeadInfo with
          | .ident .., .original .. => acc.push (ctx, ti, name, r)
          | _, _ => acc
        | _, _ => acc
      | _ => acc
    for (ctx, ti, name, r) in found do
      let j ← entry ctx ti name r
      unless j.isNull do lines := lines.push j.compress
    -- Tactics written in the file (not what notation expands to), with their goals.
    let tactics := tree.foldInfo (init := #[]) fun ctx info acc =>
      match info with
      | .ofTacticInfo ti =>
        match info.range?, ti.stx.getHeadInfo with
        | some r, .original .. => acc.push (ctx, ti, r)
        | _, _ => acc
      | _ => acc
    for (ctx, ti, r) in tactics do
      let before ← goals ctx ti.mctxBefore ti.goalsBefore
      let after ← goals ctx ti.mctxAfter ti.goalsAfter
      lines := lines.push (Json.mkObj [("goals", true), ("start", r.start.byteIdx), ("end", r.stop.byteIdx),
        ("before", before), ("after", after)]).compress
  IO.FS.writeFile out (String.intercalate "\n" lines.toList ++ "\n")
  return if errors then 1 else 0
