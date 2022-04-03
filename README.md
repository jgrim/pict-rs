# pict-rs
_a simple image hosting service_

## Links
- Find the code on [gitea](https://git.asonix.dog/asonix/pict-rs)
- Join the discussion on [matrix](https://matrix.to/#/#pictrs:matrix.asonix.dog?via=matrix.asonix.dog)
- Hit me up on [mastodon](https://masto.asonix.dog/@asonix)

## Usage
### Running
```
pict-rs 0.3.1

USAGE:
    pict-rs [FLAGS] [OPTIONS] [SUBCOMMAND]

FLAGS:
    -h, --help                     Prints help information
    -s, --skip-validate-imports    Whether to skip validating images uploaded via the internal import API
    -V, --version                  Prints version information

OPTIONS:
    -a, --addr <addr>                                          The address and port the server binds to.
        --api-key <api-key>
            An optional string to be checked on requests to privileged endpoints

    -c, --config-file <config-file>                            Path to the pict-rs configuration file
        --console-buffer-capacity <console-buffer-capacity>
            Specify the number of events the console subscriber is allowed to buffer

    -f, --filters <filters>...
            An optional list of filters to permit, supports 'identity', 'thumbnail', 'resize', 'crop', and 'blur'

    -i, --image-format <image-format>
            An optional image format to convert all uploaded files into, supports 'jpg', 'png', and 'webp'

    -m, --max-file-size <max-file-size>
            Specify the maximum allowed uploaded file size (in Megabytes)

        --max-image-area <max-image-area>                      Specify the maximum area in pixels allowed in an image
        --max-image-height <max-image-height>                  Specify the maximum width in pixels allowed on an image
        --max-image-width <max-image-width>                    Specify the maximum width in pixels allowed on an image
        --migrate-file <migrate-file>                          Path to a file defining a store migration
    -o, --opentelemetry-url <opentelemetry-url>
            Enable OpenTelemetry Tracing exports to the given OpenTelemetry collector

    -p, --path <path>                                          The path to the data directory, e.g. data/
        --sled-cache-capacity <sled-cache-capacity>
            Specify the number of bytes sled is allowed to use for it's cache


SUBCOMMANDS:
    file-store    
    help          Prints this message or the help of the given subcommand(s)
    s3-store
```

```
pict-rs-file-store 0.3.1

USAGE:
    pict-rs file-store [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --path <path>    Path in which pict-rs will create it's 'files' directory
```

```
pict-rs-s3-store 0.3.1

USAGE:
    pict-rs s3-store [OPTIONS] --bucket-name <bucket-name> --region <region>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --access-key <access-key>            
        --bucket-name <bucket-name>          Name of the bucket in which pict-rs will store images
        --region <region>                    Region in which the bucket exists, can be an http endpoint
        --secret-key <secret-key>            
        --security-token <security-token>    
        --session-token <session-token>
```

See [`pict-rs.toml`](https://git.asonix.dog/asonix/pict-rs/src/branch/main/pict-rs.toml) and 
[`migrate.toml`](https://git.asonix.dog/asonix/pict-rs/src/branch/main/migrate.toml) for more
configuration

#### Example:
Running on all interfaces, port 8080, storing data in /opt/data
```
$ ./pict-rs -a 0.0.0.0:8080 -p /opt/data
```
Running locally, port 9000, storing data in data/, and converting all uploads to PNG
```
$ ./pict-rs -a 127.0.0.1:9000 -p data/ -f png
```
Running locally, port 8080, storing data in data/, and only allowing the `thumbnail` and `identity` filters
```
$ ./pict-rs -a 127.0.0.1:8080 -p data/ -w thumbnail identity
```
Running from a configuration file
```
$ ./pict-rs -c ./pict-rs.toml
```
Migrating between storage backends
```
$ ./pict-rs -p ./data --migrate-file ./migrate.toml
```

#### Docker
Run the following commands:
```
# Create a folder for the files (anywhere works)
$ mkdir ./pict-rs
$ cd ./pict-rs
$ mkdir -p volumes/pictrs
$ sudo chown -R 991:991 volumes/pictrs
$ wget https://git.asonix.dog/asonix/pict-rs/raw/branch/main/docker/prod/docker-compose.yml
$ sudo docker-compose up -d
```
###### Note
- pict-rs makes use of the system's temporary folder. This is generally `/tmp` on linux
- pict-rs makes use of an imagemagick security policy at
    `/usr/lib/ImageMagick-$VERSION/config-Q16HDRI/policy.xml`

#### Docker Development
The development system loads a rust environment inside a docker container with the neccessary
dependencies already present
```
$ git clone https://git.asonix.dog/asonix/pict-rs
$ cd pict-rs/docker/dev
$ ./dev.sh
$ check # runs cargo check
$ build # runs cargo build
```
Development environments are provided for amd64, arm32v7, and arm64v8. By default `dev.sh` will load
into the contianer targetting amd64, but arch arguments can be passed to change the target.
```
$ ./dev.sh arm32v7
$ build
```
##### Note
Since moving to calling out to ffmpeg, imagemagick, and exiftool's binaries instead of binding
directly, the dev environment now only contains enough to build static binaries, but not run the
pict-rs program. I have personally been using alpine and arch linux to test the results. Here's how
I have been doing it:
###### With Arch
```
$ sudo docker run --rm -it -p 8080:8080 -v "$(pwd):/mnt" archlinux:latest
# pacman -Syu imagemagick ffmepg perl-image-exiftool
# ln -s /usr/bin/vendor_perl/exiftool /usr/bin/exiftool
# cp /mnt/docker/prod/root/usr/lib/ImageMagick-7.0.11/config-Q16HDRI/policy.xml /usr/lib/ImageMagick-7.1.0/config-Q16HDRI/
# RUST_LOG=debug /mnt/target/x86_64-unknown-linux-musl/debug/pict-rs -p /mnt/data
```
###### With Alpine
```
$ sudo docker run --rm -it -p 8080:8080 -v "$(pwd):/mnt alpine:3.14
# apk add imagemagick ffmpeg exiftool
# cp /mnt/docker/prod/root/usr/lib/ImageMagick-7.0.11/config-Q16HDRI/policy.xml /usr/lib/ImageMagick-7.0.11/config-Q16HDRI/
# RUST_LOG=debug /mnt/target/x86_64-unknown-linux-musl/debug/pict-rs -p /mnt/data
```

### API
pict-rs offers the following endpoints:
- `POST /image` for uploading an image. Uploaded content must be valid multipart/form-data with an
    image array located within the `images[]` key

    This endpoint returns the following JSON structure on success with a 201 Created status
    ```json
    {
        "files": [
            {
                "delete_token": "JFvFhqJA98",
                "file": "lkWZDRvugm.jpg"
            },
            {
                "delete_token": "kAYy9nk2WK",
                "file": "8qFS0QooAn.jpg"
            },
            {
                "delete_token": "OxRpM3sf0Y",
                "file": "1hJaYfGE01.jpg"
            }
        ],
        "msg": "ok"
    }
    ```
- `GET /image/download?url=...` Download an image from a remote server, returning the same JSON
    payload as the `POST` endpoint
- `GET /image/original/{file}` for getting a full-resolution image. `file` here is the `file` key from the
    `/image` endpoint's JSON
- `GET /image/details/original/{file}` for getting the details of a full-resolution image.
    The returned JSON is structured like so:
    ```json
    {
        "width": 800,
        "height": 537,
        "content_type": "image/webp",
        "created_at": [
            2020,
            345,
            67376,
            394363487
        ]
    }
    ```
- `GET /image/process.{ext}?src={file}&...` get a file with transformations applied.
    existing transformations include
    - `identity=true`: apply no changes
    - `blur={float}`: apply a gaussian blur to the file
    - `thumbnail={int}`: produce a thumbnail of the image fitting inside an `{int}` by `{int}`
        square using raw pixel sampling
    - `resize={int}`: produce a thumbnail of the image fitting inside an `{int}` by `{int}` square
        using a Lanczos2 filter. This is slower than sampling but looks a bit better in some cases
    - `crop={int-w}x{int-h}`: produce a cropped version of the image with an `{int-w}` by `{int-h}`
        aspect ratio. The resulting crop will be centered on the image. Either the width or height
        of the image will remain full-size, depending on the image's aspect ratio and the requested
        aspect ratio. For example, a 1600x900 image cropped with a 1x1 aspect ratio will become 900x900. A
        1600x1100 image cropped with a 16x9 aspect ratio will become 1600x900.

    Supported `ext` file extensions include `png`, `jpg`, and `webp`

    An example of usage could be
    ```
    GET /image/process.jpg?src=asdf.png&thumbnail=256&blur=3.0
    ```
    which would create a 256x256px JPEG thumbnail and blur it
- `GET /image/details/process.{ext}?src={file}&...` for getting the details of a processed image.
    The returned JSON is the same format as listed for the full-resolution details endpoint.
- `DELETE /image/delete/{delete_token}/{file}` or `GET /image/delete/{delete_token}/{file}` to
    delete a file, where `delete_token` and `file` are from the `/image` endpoint's JSON


The following endpoints are protected by an API key via the `X-Api-Token` header, and are disabled
unless the `--api-key` option is passed to the binary or the PICTRS_API_KEY environment variable is
set.

A secure API key can be generated by any password generator.
- `POST /internal/import` for uploading an image while preserving the filename. This should not be
    exposed to the public internet, as it can cause naming conflicts with saved files. The upload
    format and response format are the same as the `POST /image` endpoint.
- `POST /internal/purge?...` Purge a file by it's filename or alias. This removes all aliases and
    files associated with the query.
    - `?file=asdf.png` purge by filename
    - `?alias=asdf.png` purge by alias

    This endpoint returns the following JSON
    ```json
    {
        "msg": "ok",
        "aliases": ["asdf.png"]
    }
    ```
- `GET /internal/aliases?...` Get the aliases for a file by it's filename or alias
    - `?file={filename}` get aliases by filename
    - `?alias={alias}` get aliases by alias

    This endpiont returns the same JSON as the purge endpoint
- `GET /internal/filename?alias={alias}` Get the filename for a file by it's alias
    This endpoint returns the following JSON
    ```json
    {
        "msg": "ok",
        "filename": "asdf.png"
    }
    ```

Additionally, all endpoints support setting deadlines, after which the request will cease
processing. To enable deadlines for your requests, you can set the `X-Request-Deadline` header to an
i128 value representing the number of nanoseconds since the UNIX Epoch. A simple way to calculate
this value is to use the `time` crate's `OffsetDateTime::unix_timestamp_nanos` method. For example,
```rust
// set deadline of 1ms
let deadline = time::OffsetDateTime::now_utc() + time::Duration::new(0, 1_000);

let request = client
    .get("http://pict-rs:8080/image/details/original/asdfghjkla.png")
    .insert_header(("X-Request-Deadline", deadline.unix_timestamp_nanos().to_string())))
    .send()
    .await;
```


## Contributing
Feel free to open issues for anything you find an issue with. Please note that any contributed code will be licensed under the AGPLv3.

## FAQ
### Question: I want to configure it with yaml instead of toml
Answer: That's not a question, but you can configure pict-rs with json, hjson, yaml, ini, or toml.
Writing configs in other formats is left as an exercise to the reader.

## License

Copyright © 2022 Riley Trautman

pict-rs is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

pict-rs is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details. This file is part of pict-rs.

You should have received a copy of the GNU General Public License along with pict-rs. If not, see [http://www.gnu.org/licenses/](http://www.gnu.org/licenses/).
