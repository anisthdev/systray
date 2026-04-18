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
source=("$pkgname-$pkgver.tar.gz::$url/releases/download/v$pkgver/$pkgname-$pkgver.tar.gz")
sha256sums=('618988f12a41320e00919a7c71ebf69ae4851526fd72508baaed758b53333aad')

build() {
  cd "$srcdir/$pkgname-$pkgver"
  cargo build --release --locked
}

package() {
  cd "$srcdir/$pkgname-$pkgver"

  install -Dm755 target/release/tray "$pkgdir/usr/bin/tray"
  install -Dm644 tray.1 "$pkgdir/usr/share/man/man1/tray.1"
  install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
