## FUNCTIONAL TEST 3
1) Create 1st blog at POST /blog
2) Create 2nd blog at POST /blog
3) Get all (2) blogs at GET /blog
4) TODO: Remove all (2) blogs at DELETE /blog/>blogID<

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
[let id1]: # ($.id)



This request creates blog without comment
```docassertrequest
POST /blog
Content-Type: application/json
{
    "title": "My Second Blog",
    "body": "This is my sample blog"
}
```

This is response for creating blog without comment
```docassertresponse
HTTP 200
Content-Type: application/json
{
    "id": "d8f7d454-c436-4e0f-9613-1d69036ad421",
    "title": "My Second Blog",
    "body": "This is my sample blog",
    "date_upd": 1707906394,
    "comments" : null
}
```
[ignore]: # ($.id)
[ignore]: # ($.date_upd)
[let id2]: # ($.id)


This request reads all blogs
```docassertrequest
GET /blog
Content-Type: application/json
{
}
```

This is response for getting all blogs
```docassertresponse
HTTP 200
Content-Type: application/json
        [
          {
            "body": "This is my sample blog",
            "comments": null,
            "date_upd": 1708421556,
            "id": "a8f376b1-382e-443b-bbfc-7daf0bbdac38",
            "title": "My First Blog"
          },
          {
            "body": "This is my sample blog",
            "comments": null,
            "date_upd": 1708421556,
            "id": "914ace04-1ad7-4c55-ab92-0876b7206430",
            "title": "My Second Blog"
          }
        ]
```
[ignore]: # ($.id)
[ignore]: # ($.date_upd)
[ignore]: # ($.comments)

