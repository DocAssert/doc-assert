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

TODO: figure out how to ignore id which can be random

```docassertresponse
HTTP 201
Content-Type: application/json
{
    "id": 1,
    "name": "test"
}
```
[ignore]: # ($.id)


## Unrelated
Some other unrelated code which should not be parsed
```rust
fn main() {
    println!("Hello, world!");
}
```