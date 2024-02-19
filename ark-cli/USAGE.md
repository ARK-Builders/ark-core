## Usage

Beginning with the following dir structure

```
├───<root_dir>
│   └───.ark
│       ├───cache
│       │   ├───metadata
│       │   └───previews
│       └───user
│           ├───properties
│           ├───scores
│           └───tags
```

First create some sample links

`ark-cli link create <root_dir> http://google.com google hi`

`ark-cli link create <root_dir> http://bing.com bing hello`

Then add some tags to the links

`ark-cli file append <root_dir> tags <resource_id> search,engine`

The same way we can append scores

`ark-cli file append <root_dir> scores <resource_id> 15`

We can also append json data

`ark-cli file append <root_dir> properties <resource_id> favorites:false,ai:true --format=json`

You can read these properties

`ark-cli file read <root_dir> properties <resource_id>`

Or the scores

`ark-cli file read <root_dir> scores <resource_id>`

You can list the entries for a storage like this

`ark-cli storage list <root_dir> properties`

For more info you can add the versions flag

`ark-cli storage list <root_dir> properties --versions=true`

Also works for file storages

`ark-cli storage list <root_dir> scores --versions=true`

List the files in the index using 

`ark-cli list <root_dir>`

`--entry=id|path|both` -> to show the path,the id or both of a resource

`--timestamp=true` -> to show or not the last modified timestamp of a resource

`--tags=true` -> to show or not the tags for every resource

`--scores=true` -> to show or not the scores for every resource

`--sort=asc|desc` -> to sort resources by asc or dsc order of scores

`--filter=query` -> to filter resources by their tags





