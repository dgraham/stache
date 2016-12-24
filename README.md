# Stache

A [Mustache] template [compiler].

[Mustache]: http://mustache.github.io
[compiler]: http://llvm.org

## Usage

```
$ stache -d app/templates/ -o templates.c --emit=ruby
$ stache -d app/templates/ -o templates.c --emit=ruby && clang-format -i -style=webkit templates.c
```

## Development

```
$ cargo test
$ cargo build
```

## License

Stache is released under the MIT license. Check the LICENSE file for details.
