# Example application to test provisioning

## Building container image

    docker build . -t exampleapp --platform=aarch64

## Export image to use it in the image registry server

    docker image save -o exampleapp.tar exampleapp:latest

