# `data-resource`

`data-resource` is a lightweight minimal dependency crate that defines the `ResourceId` trait. This trait specifies constraints for any type that represents a `ResourceId`. It is used by other ARK crates to make the index generic over the resource ID type and hash function used.

## Example Implementations

To see example implementations of the `ResourceId` trait, please refer to the [`dev-hash`](../dev-hash) crate.
