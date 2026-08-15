PREFIX ?= /usr/local
CARGO ?= cargo

.PHONY: build install uninstall test plugin-test validate fmt clippy bundle clean

build:
	$(CARGO) build --release

install: build
	install -Dm755 target/release/wireview-pro2-qs $(DESTDIR)$(PREFIX)/bin/wireview-pro2-qs

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/wireview-pro2-qs

bundle:
	$(CARGO) build --release --locked --target x86_64-unknown-linux-musl
	install -Dm755 target/x86_64-unknown-linux-musl/release/wireview-pro2-qs omarchy/bin/wireview-pro2-qs

test:
	$(CARGO) test --all-targets

plugin-test:
	node omarchy/model.test.mjs

validate:
	omarchy plugin validate .
	qmllint -I "$(OMARCHY_PATH)/shell" omarchy/BarWidget.qml omarchy/Panel.qml

fmt:
	$(CARGO) fmt

clippy:
	$(CARGO) clippy --all-targets -- -D warnings

clean:
	$(CARGO) clean
