"""Tooling suite: every component mirror builds and tests alone; negative controls for the mirror rules first."""
from pathlib import Path
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import component_mirrors as mirrors


def write(root, path, text):
    file = root / path
    file.parent.mkdir(parents=True, exist_ok=True)
    file.write_text(text)


def negative_controls():
    original = mirrors.ROOT
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory).resolve()
        mirrors.ROOT = root
        try:
            write(root, 'components/x/Cargo.toml', '[package]\nname = "veyndo-x"\n')
            write(root, 'components/x/src/ports.rs', 'pub trait Served {}\npub trait Tested {}\npub trait Missing {}\n')
            write(root, 'components/x/src/lib.rs', 'impl Served for Here {}\n#[cfg(test)]\nmod tests {}\n')
            write(root, 'components/x/tests/adapters.rs', 'impl crate::ports::Tested for Fake {}\n')
            found = mirrors.port_adapters(['components/x'])
            assert found == {'veyndo-x::Served': 'component', 'veyndo-x::Tested': 'test',
                             'veyndo-x::Missing': None}, found
            # A component that reaches the server crate cannot be mirrored.
            write(root, 'components/x/Cargo.toml', '[package]\nname = "veyndo-x"\n[dependencies]\nveyndo = { path = "../.." }\n')
            try:
                mirrors.closure(['components/x'])
            except AssertionError as error:
                assert 'server crate' in str(error)
            else:
                raise AssertionError('A dependency on the server crate was accepted')
            # A path dependency outside the monorepo is refused, not silently copied.
            write(root, 'components/x/Cargo.toml', '[package]\nname = "veyndo-x"\n[dependencies]\nfar = { path = "../../../far" }\n')
            try:
                mirrors.closure(['components/x'])
            except (AssertionError, ValueError):
                pass
            else:
                raise AssertionError('A path dependency outside the monorepo was accepted')
        finally:
            mirrors.ROOT = original
    print('PASS mirror negative controls: port adapter kinds, server dependency and escaping path refused')


if __name__ == '__main__':
    negative_controls()
    subprocess.run([sys.executable, str(mirrors.ROOT / 'scripts/component_mirrors.py'), '--check'], check=True)
