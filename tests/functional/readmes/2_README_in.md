## FUNCTIONAL TEST 2
### Remark: This test is checking update operation behavior
1) Create blog at POST /blog
2) Update  blog at PUT /blog/>blogID<
3) Get all (1) blogs at GET /blog
4) Delete blog at DELETE /blog/>blogID<


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



This request creates blog comment
```docassertrequest
PUT /blog/`id`
Content-Type: application/json
{
    "title": "My First Blog - UPDATED",
    "body": "This is my sample blog - UPDATED"
}
```

This is response for creating blog
```docassertresponse
HTTP 200
Content-Type: application/json
{
    "id": "d8f7d454-c436-4e0f-9613-1d69036ad421",
    "title": "My First Blog- UPDATED",
    "body": "This is my sample blog - UPDATED",
    "date_upd": 1707906394
}
```
[ignore]: # ($.id)
[ignore]: # ($.date_upd)
[ignore]: # ($.comments)


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
    "title": "My First Blog-UPDATED",
    "body": "This is my sample blog - UPDATED",
    "date_upd": 1707906394,
    "comments" : null
}
```
[ignore]: # ($.id)
[ignore]: # ($.date_upd)
[ignore]: # ($.comments)

