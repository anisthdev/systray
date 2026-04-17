# Maintainer: Asif Arbaj <asifarbaj@gmail.com>

pkgname=systray
pkgver=0.1.0
pkgrel=1
pkgdesc="System tray helper for shell scripts"
arch=('x86_64')
url="https://github.com/anisthdev/systray"
license=('MIT')
depends=('dbus')
makedepends=('cargo' 'rust')
provides=('tray')
source=("git+https://github.com/anisthdev/systray.git")
sha256sums=('SKIP')

build() {
  cd "$srcdir/$pkgname"
  cargo build --release --locked
}

package() {
  cd "$srcdir/$pkgname"

  install -Dm755 target/release/systray "$pkgdir/usr/bin/tray"
  install -Dm644 tray.1 "$pkgdir/usr/share/man/man1/tray.1"
  install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
