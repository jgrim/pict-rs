# pict-rs
_a simple image hosting service_

## Usage
### Running
```
pict-rs 0.2.6

USAGE:
    pict-rs [FLAGS] [OPTIONS] --path <path>

FLAGS:
    -h, --help                     Prints help information
    -s, --skip-validate-imports    Whether to skip validating images uploaded via the internal import API
    -V, --version                  Prints version information

OPTIONS:
    -a, --addr <addr>                      The address and port the server binds to. [env: PICTRS_ADDR=]  [default:
                                           0.0.0.0:8080]
        --api-key <api-key>                An optional string to be checked on requests to privileged endpoints [env:
                                           PICTRS_API_KEY=]
    -f, --format <format>                  An optional image format to convert all uploaded files into, supports 'jpg',
                                           'png', and 'webp' [env: PICTRS_FORMAT=]
    -m, --max-file-size <max-file-size>    Specify the maximum allowed uploaded file size (in Megabytes) [env:
                                           PICTRS_MAX_FILE_SIZE=]  [default: 40]
    -p, --path <path>                      The path to the data directory, e.g. data/ [env: PICTRS_PATH=]
    -w, --whitelist <whitelist>...         An optional list of filters to whitelist, supports 'identity', 'thumbnail',
                                           and 'blur' [env: PICTRS_FILTER_WHITELIST=]
```

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

#### Docker
Run the following commands:
```
# Create a folder for the files (anywhere works)
$ mkdir ./pict-rs
$ cd ./pict-rs
$ mkdir -p volumes/pictrs
$ sudo chown -R 991:991 volumes/pictrs
$ wget https://git.asonix.dog/asonix/pict-rs/raw/branch/master/docker/prod/docker-compose.yml
$ sudo docker-compose up -d
```

###### Note
- pict-rs makes use of the system's temporary folder. This is generally `/tmp` on linux
- pict-rs makes use of a default imagemagick security policy at
    `/usr/local/lib/ImageMagick-$VERSION/config-Q16HDRI/policy.xml`

#### Docker Development
The development system loads a rust environment inside a docker container with the neccessary
dependencies already present
```
$ git clone https://git.asonix.dog/asonix/pict-rs
$ cd pict-rs/docker/dev
$ ./dev.sh
$ check # runs cargo check
$ build # runs cargo build
$ run -p data/
```
Development environments are provided for amd64, arm32v7, and arm64v8. By default `dev.sh` will load
into the contianer targetting amd64, but arch arguments can be passed to change the target.
```
$ ./dev.sh arm32v7
$ build

# note:
# This command may not work unless qemu-user-static has been configured for your docker instance
$ run -p data/
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
- `GET /image/process.{ext}?src={file}&...` get a file with transformations applied.
    existing transformations include
    - `identity=true`: apply no changes
    - `blur={float}`: apply a gaussian blur to the file
    - `thumbnail={int}`: produce a thumbnail of the image fitting inside an `{int}` by `{int}` square

    Supported `ext` file extensions include `png`, `jpg`, and `webp`

    An example of usage could be
    ```
    GET /image/process.jpg?src=asdf.png&thumbnail=256&blur=3.0
    ```
    which would create a 256x256px JPEG thumbnail and blur it
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


## Contributing
Feel free to open issues for anything you find an issue with. Please note that any contributed code will be licensed under the AGPLv3.

## License

Copyright © 2020 Riley Trautman

pict-rs is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

pict-rs is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details. This file is part of pict-rs.

You should have received a copy of the GNU General Public License along with pict-rs. If not, see [http://www.gnu.org/licenses/](http://www.gnu.org/licenses/).
