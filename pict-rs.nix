{ exiftool
, ffmpeg_5-full
, imagemagick
, lib
, makeWrapper
, nixosTests
, protobuf
, rustPlatform
, Security
, stdenv
}:

rustPlatform.buildRustPackage {
  pname = "pict-rs";
  version = "0.3.2";
  src = ./.;
  cargoLock = {
    lockFile = ./Cargo.lock;
    outputHashes = {
      "aws-creds-0.29.1" = "bwDFmDPThMLrpaB7cAj/2/vJKhbX6/DqgcIRBVKSZhg=";
    };
  };

  PROTOC = "${protobuf}/bin/protoc";
  PROTOC_INCLUDE = "${protobuf}/include";

  nativeBuildInputs = [ makeWrapper ];
  buildInputs = lib.optionals stdenv.isDarwin [ Security ];

  postInstall = ''
    wrapProgram $out/bin/pict-rs \
        --prefix PATH : "${lib.makeBinPath [ imagemagick ffmpeg_5-full exiftool ]}"
  '';

  passthru.tests = { inherit (nixosTests) pict-rs; };

  meta = with lib; {
    description = "A simple image hosting service";
    homepage = "https://git.asonix.dog/asonix/pict-rs";
    license = with licenses; [ agpl3Plus ];
  };
}
