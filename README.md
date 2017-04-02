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
```

Benchmark against [erubi] with:

[erubi]: https://github.com/jeremyevans/erubi

```
$ gem install benchmark-ips erubi
$ cargo test bench -- --ignored --nocapture
```

## License

Stache is released under the MIT license. Check the LICENSE file for details.
