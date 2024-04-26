## FUNCTIONAL TEST 4
1) Get 5 times the status of faulty endpoint, expect "faulty": true at 5th time

```docassertrequest
GET /status
```

This is response for faulty endpoint
```docassertresponse
HTTP 200
Content-Type: application/json
{
    "faulty": true
}
```
[retry]: # (5, 100)