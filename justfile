default:
    echo 'Hello, world!'

init:
    cargo binstall cargo-nextest
    cargo binstall cargo-insta --locked
    cargo binstall cargo-whatfeatures
    cargo binstall cargo-codspeed
    cargo binstall cargo-chef
    cargo binstall cargo-audit --locked
    cargo binstall cargo-shear