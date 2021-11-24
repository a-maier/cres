# This script takes care of building your crate and packaging it for release

set -ex

main() {
    local src=$(pwd) \
          stage=

    case $TRAVIS_OS_NAME in
        linux)
            stage=$(mktemp -d)
            ;;
        osx)
            stage=$(mktemp -d -t tmp)
            ;;
    esac

    test -f Cargo.lock || cargo generate-lockfile

    cross rustc --bin cres --target $TARGET --release

    ls target/$TARGET/release/*
    cp target/$TARGET/release/cres $stage/
    cp target/$TARGET/release/libcres.a $stage/
    if [ -f target/$TARGET/release/libcres.so ]; then
        cp target/$TARGET/release/libcres.so $stage/
    fi

    cd $stage
    tar czf $src/$CRATE_NAME-$TRAVIS_TAG-$TARGET.tar.gz *
    cd $src

    rm -rf $stage
}

main
