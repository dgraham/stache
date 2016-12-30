# Stache

A [Mustache] template compiler.

[Mustache]: http://mustache.github.io

## Usage

```
$ stache -d app/templates/ -o stache.c --emit=ruby
$ stache -d app/templates/ -o stache.c --emit=ruby && clang-format -i -style=webkit stache.c
```

## Development

```
$ git submodule update --init
$ cargo test
$ cargo build

# Test Mustache specification compliance (ignored until whitespace tests pass)
$ cargo test -- --ignored
```

## License

Stache is released under the MIT license. Check the LICENSE file for details.
