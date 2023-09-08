# docker

```bash
# build
$ docker build -t lgtn .

# key gen
$ docker run -v /root/.lightning/:/root/.lightning/ lgtn /usr/lightning/lgtn keys generate

# run
$ docker run -v /root/.lightning/:/root/.lightning/ lgtn /usr/lightning/lgtn -v run
```
