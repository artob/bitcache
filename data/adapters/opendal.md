#### OpenDAL Adapter

##### Azure Blob Storage ([`azblob`]) Service

```dotenv
BITCACHE_URL=opendal+azblob://my-container
```

<details>
<summary>Configuration for Floci AZ</summary>

###### Configuration for [Floci AZ](https://github.com/floci-io/floci-az)

```dotenv
BITCACHE_URL=opendal+azblob://my-container?endpoint=http://localhost:4577/devstoreaccount1&skip_signature=true
```
</details>

##### File System Service ([`fs`])

```dotenv
BITCACHE_URL=opendal+fs:///tmp/bitcache
```

##### FTP Service ([`ftp`])

```dotenv
BITCACHE_URL=opendal+ftp://localhost
```

<details>
<summary>Configuration for pyftpdlib</summary>

###### Configuration for [pyftpdlib](https://github.com/giampaolo/pyftpdlib)

```dotenv
BITCACHE_URL=opendal+ftp://127.0.0.1:2121?user=anonymous&password=jhacker@example.org
```
</details>

##### Google Cloud Storage Service ([`gcs`])

```dotenv
BITCACHE_URL=opendal+gcs://my-bucket/my-root
```

<details>
<summary>Configuration for Floci GCP</summary>

###### Configuration for [Floci GCP](https://github.com/floci-io/floci-gcp)

```dotenv
BITCACHE_URL=opendal+gcs://my-bucket/my-root?endpoint=http://localhost:4588&skip_signature=true
```
</details>

##### HTTP Service ([`http`])

```dotenv
BITCACHE_URL=opendal+http://localhost:8000
```

##### Memcached Service ([`memcached`])

```dotenv
BITCACHE_URL=opendal+memcached://localhost:11211
```

##### Memory Service ([`memory`])

```dotenv
BITCACHE_URL=opendal+memory://
```

##### MongoDB Service ([`mongodb`])

```dotenv
BITCACHE_URL=opendal+mongodb://localhost:27017/my-database/my-collection
```

##### Redis Service ([`redis`])

```dotenv
BITCACHE_URL=opendal+redis://localhost:6379
```

##### S3 Service ([`s3`])

```dotenv
BITCACHE_URL=opendal+s3://my-bucket
```

<details>
<summary>Configuration for Floci AWS</summary>

###### Configuration for [Floci AWS](https://github.com/floci-io/floci)

```dotenv
BITCACHE_URL=opendal+s3://my-bucket?region=us-east-1&endpoint=http://localhost:4566&skip_signature=true
```
</details>

##### SFTP Service ([`sftp`])

```dotenv
BITCACHE_URL=opendal+sftp://my-host
```

##### Sled Service ([`sled`])

```dotenv
BITCACHE_URL=opendal+sled:///tmp/bitcache
```

##### Miscellaneous Services

OpenDAL supports dozens more additional [services](https://opendal.apache.org/services/);
however, if we haven't validated them yet, we won't have a feature flag for
them nor URL scheme support in Bitcache directly. (Submit a pull request to add
support for your favorite service!)

[`azblob`]: https://opendal.apache.org/services/azblob/
[`fs`]: https://opendal.apache.org/services/fs/
[`ftp`]: https://opendal.apache.org/services/ftp/
[`gcs`]: https://opendal.apache.org/services/gcs/
[`http`]: https://opendal.apache.org/services/http/
[`memcached`]: https://opendal.apache.org/services/memcached/
[`memory`]: https://opendal.apache.org/services/memory/
[`mongodb`]: https://opendal.apache.org/services/mongodb/
[`redis`]: https://opendal.apache.org/services/redis/
[`s3`]: https://opendal.apache.org/services/s3/
[`sftp`]: https://opendal.apache.org/services/sftp/
[`sled`]: https://opendal.apache.org/services/sled/
