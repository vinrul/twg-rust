# WebSocket Baseline

Captured at: 2026-11-04T07:45:12.724Z

- Channel: `twg-baseline`
- Subscribed: **true**
- Allowed publish HTTP: **201**
- Cross-satker publish HTTP: **403**
- Errors: **0**

## Publication diterima

```json
{
  "operasi_id": "33333333-3333-4333-8333-333333333331",
  "skenario_id": "44444444-4444-4444-8444-444444444441",
  "marker_id": "55555555-5555-4555-8555-555555555551",
  "command": "MOVE",
  "data": "{\"x\":115.2251,\"y\":-8.6481,\"z\":0,\"source\":\"ws-baseline\"}",
  "is_save": 1,
  "satker_id": "11111111-1111-4111-8111-111111111111"
}
```

## Cross-satker response

```json
{
  "http_status": 403,
  "body": {
    "code": 403,
    "message": "cross-satker command not allowed"
  }
}
```
