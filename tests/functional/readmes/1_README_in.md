## FUNCTIONAL TEST 1
### Remark: This test is checking 'let id' that value changes across tests
1) Create blog at POST /blog
2) Delete blog at DELETE /blog/>blogID<
3) Create new blog at POST /blog
4) Delete newly created blog at DELETE /blog/>blogID<


This request creates blog without comment
```docassertrequest
POST /blog
Content-Type: application/json
{
    "title": "My First Blog",
    "body": "This is my sample blog"
}
```

This is response for creating blog without comment
```docassertresponse
HTTP 200
Content-Type: application/json
{
    "id": "d8f7d454-c436-4e0f-9613-1d69036ad421",
    "title": "My First Blog",
    "body": "This is my sample blog",
    "date_upd": 1707906394,
    "comments" : null
}
```
[ignore]: # ($.id)
[ignore]: # ($.date_upd)
[let id]: # ($.id)



This request deletes blog with given id
```docassertrequest
DELETE /blog/`id`
Content-Type: application/json
{
}
```

This is response for deleting blog
```docassertresponse
HTTP 200
Content-Type: application/json
{
    "id": "d8f7d454-c436-4e0f-9613-1d69036ad421",
    "title": "My First Blog",
    "body": "This is my sample blog",
    "date_upd": 1707906394,
    "comments" : null
}
```
[ignore]: # ($.id)
[ignore]: # ($.date_upd)
[ignore]: # ($.comments)



This request creates blog with comment ignored in response
```docassertrequest
POST /blog
Content-Type: application/json
{
    "title": "My Second Blog",
    "body": "This is my second sample blog"
}
```

This is response for creating blog
```docassertresponse
HTTP 200
Content-Type: application/json
{
    "id": "d8f7d454-c436-4e0f-9613-1d69036ad421",
    "title": "My Second Blog",
    "body": "This is my second sample blog",
    "date_upd": 1707906394
}
```
[ignore]: # ($.id)
[ignore]: # ($.date_upd)
[ignore]: # ($.comments)
[let id]: # ($.id)


This request deletes blog with given id
```docassertrequest
DELETE /blog/`id`
Content-Type: application/json
{
}
```

This is response for deleting blog
```docassertresponse
HTTP 200
Content-Type: application/json
{
    "id": "d8f7d454-c436-4e0f-9613-1d69036ad421",
    "title": "My Second Blog",
    "body": "This is my second sample blog",
    "date_upd": 1707906394,
    "comments" : null
}
```
[ignore]: # ($.id)
[ignore]: # ($.date_upd)
[ignore]: # ($.comments)
