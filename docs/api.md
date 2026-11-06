---
title: API
nav_order: 3
---

# The API

verkodo serves its API on the device itself: `verkodo serve` listens on `http://127.0.0.1:8080`, the web
app at `/`, the API under `/api/v1`. Requests and answers are JSON; an error is `{"error": "…"}` with a fitting
status.

The full description in OpenAPI is [`api/openapi.json`](https://github.com/verkodo/verkodo/blob/main/api/openapi.json)
– generated from the code by `verkodo openapi`, and checked by CI to match it. A running server gives the same at
`/api/v1/openapi.json`.

| Method | Path | What |
|---|---|---|
