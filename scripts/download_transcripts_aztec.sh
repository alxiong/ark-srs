#!/bin/sh
# Downloads the ignition trusted setup transcripts.
#
# To download all transcripts.
#  ./download_transcript.sh
#
# To download a range of transcripts, e.g. 0, 1 and 2.
#  ./download_transcript.sh 2
#
# If a checksums file is available, it will be used to validate if a download is required
# and also check the validity of the downloaded transcripts. If not the script downloads
# whatever is requested but does not check the validity of the downloads.
#
# original credit: Zac Williamson
set -e

SCRIPT_DIR="$(dirname $0)"
ROOT_DIR="$(dirname $SCRIPT_DIR)"

TRANSCRIPT_URL="https://aztec-ignition.s3.eu-west-2.amazonaws.com/MAIN+IGNITION/sealed"
SEALED_TRANSCRIPT_DIR="$ROOT_DIR/data/aztec20"

mkdir -p $SEALED_TRANSCRIPT_DIR
cd $SEALED_TRANSCRIPT_DIR
NUM=${1:-19}

checksum() {
    grep transcript${1}.dat checksums | sha256sum -c
    return $?
}

download() {
    echo "Downloading to $SEALED_TRANSCRIPT_DIR/transcript${1}.dat ..."
    curl https://aztec-ignition.s3-eu-west-2.amazonaws.com/MAIN%20IGNITION/sealed/transcript${1}.dat > ./transcript${1}.dat
}

echo "Downloading or checking (if already downloaded) CRS (it might take a long time as there are 20 files each 320M in size.) ..."
for TRANSCRIPT in $(seq 0 $NUM); do
    NUM=$(printf %02d $TRANSCRIPT)
    if [ -f checksums ]; then
        checksum $NUM && continue
        download $NUM
        checksum $NUM || exit 1
    else
        download $NUM
    fi
done
