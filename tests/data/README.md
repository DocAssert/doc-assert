Example documentation used for unit tests

## User

This request creates user

```docassertrequest
POST /api/user
Content-Type: application/json
{
    "name": "test"
}
```

And as a response we should get something like this

```docassertresponse
HTTP 201
Content-Type: application/json
{
    "id": 1,
    "name": "test"
}
```

[ignore]: # ($.id)

[let name]: # ($.name)

[retry]: # (3, 4500)

Now let's add another user

```docassertrequest
POST /api/user
Content-Type: application/json
{
    "name": "test"
}
```

And as a response we should get something like this

```docassertresponse
HTTP 201
Content-Type: application/json
{
    "id": 1,
    "name": "tes1t"
}
```

## Unrelated

Some other unrelated code which should not be parsed

```rust
fn main() {
    println!("Hello, world!");
}
```