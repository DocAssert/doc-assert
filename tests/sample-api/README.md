## User
This request creates blog
```docassertrequest
POST /blog/
Content-Type: application/json
{
    "title": "Title",
    "body": `body`
}
```

And as a response we should get something like this

```docassertresponse
HTTP 200
Content-Type: application/json
{
    "id": 1,
    "title": "Title",
    "body": `body`,
    "date_upd": 1,
    "comments": null
}
```
[ignore]: # ($.id)
[ignore]: # ($.date_upd)
[let id]: # ($.id)

And then to get the blog
```docassertrequest
GET /blog/`id`
Content-Type: application/json
```

```docassertresponse
HTTP 200
Content-Type: application/json
{
    "id": `id`,
    "title": "Title",
    "body": `body`,
    "date_upd": 1,
    "comments": null
}
```
[ignore]: # ($.date_upd)


And then add another blog

```docassertrequest
POST /blog/
Content-Type: application/json
{
    "title": "Title 2",
    "body": `body`
}
```

With expected response

```docassertresponse
HTTP 200
Content-Type: application/json
{
    "id": 1,
    "title": "Title 2",
    "body": `body`,
    "date_upd": 1,
    "comments": null
}
```
[ignore]: # ($.id)
[ignore]: # ($.date_upd)
[let id2]: # ($.id)

After listing the blogs:

```docassertrequest
GET /blog/
Content-Type: application/json
```

We should obtain the list of blogs

```docassertresponse
HTTP 200
Content-Type: application/json
[
    {
        "id": 1,
        "title": "Title",
        "body": `body`,
        "date_upd": 1,
        "comments": null
    }, 
    {
        "id": 1,
        "title": "Title 2",
        "body": `body`,
        "date_upd": 1,
        "comments": null
    }
]
```
[ignore-order]: # ($)
[ignore]: # ($[:].id)
[ignore]: # ($[:].date_upd)

And cleanup!
```docassertrequest
DELETE /blog/`id`
Content-Type: application/json
```

```docassertresponse
HTTP 200
Content-Type: application/json
{
    "id": `id`,
    "title": "Title",
    "body": `body`,
    "date_upd": 1,
    "comments": null
}
```
[ignore]: # ($.date_upd)


```docassertrequest
DELETE /blog/`id2`
Content-Type: application/json
```

```docassertresponse
HTTP 200
Content-Type: application/json
{
    "id": `id2`,
    "title": "Title 2",
    "body": `body`,
    "date_upd": 1,
    "comments": null
}
```
[ignore]: # ($.date_upd)