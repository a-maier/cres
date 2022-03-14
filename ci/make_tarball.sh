#$/bin/sh

set -ex

mkdir $PROJECT $PROJECT/bin $PROJECT/lib $PROJECT/include
cp target/$1/release/$PROJECT $PROJECT/bin/
cp target/$1/release/lib$PROJECT.* $PROJECT/lib/
cp target/$1/release/build/*/out/cres.h $PROJECT/include/
tar czf $PROJECT.tar.gz $PROJECT
rm -r $PROJECT
