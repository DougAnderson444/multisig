# Multisig

## Current Status 

This create currently supports the following digital signature protocols.

* EdDSA (ed25519)
 * AttrView, SigDataView
* Es256K (secp256k1)
 * AttrView, SigDataView
* Bls12-381 G1/G2
 * AttrView, SigDataView, ThresholdAttrView, ThresholdView

This crate also supports converting to/from SSH format digital signatures using
the [`ssh-key`](https://crates.io/crates/ssh-key) crate. This gives full 
OpenSSH compatibility for reading in OpenSSH serialized keys and converting 
them to Multikey format. This even includes non-standard SSH key protocols 
such as Es256K and Bls12-381 signatures through the use of 
[RFC 4251](https://www.rfc-editor.org/rfc/rfc4251.html#page-10) standard for 
"additional algorithms" names using the "@multisig" domain suffix. For
instance, using this crate, an Es256K Multisig converted to an SSH format
signature has the name "secp256k1@multisig". A BLS12-381 G1 signature share
converted to SSH format has the name "bls12_381-g1-share@multsig".

## Introduction

This is a Rust implementation of a multicodec format for digital signatures.
The design of the format is intentionally abstract to support any kind of
digital signature data for any protocol. This format should best be thought of
as a container of signature data with abstract, protocol-specific views but a
generic and self-describing data storage format. 

Every piece of data in a serialized Multisig object either has a known-fixed
size or a self-describing variable size such that software processing these
objects do not need to support all digital signature protocols to be able to
accurately calculate the size of the serialized object and skip over it if
needed.

The only operations that can be executed on a Multisig object those that return
the stored data and the threshold signature operations for accumulating and 
combining signature shares. Any operation that involves a cryptographic key is 
found in the [`Multikey`](https://github.com/cryptidtech/multikey.git) 
companion crate.

## Multisig Format 

```
     key codec        signature attributes
         |                     |
         v                     v
0x39 <varuint> <message> <attributes>
^                  ^
|                  |
multisig    optional combined
sigil       signature message

<message> ::= <varbytes>

                         variable number of attributes
                                       |
                            ______________________
                           /                      \
<attributes> ::= <varuint> N(<varuint>, <varbytes>)
                     ^           ^          ^
                    /           /           |
            count of      attribute     attribute
          attributes     identifier       value


<varbytes> ::= <varuint> N(OCTET)
                   ^        ^
                  /          \
          count of            variable number
            octets            of octets
```

The Multisig format allows tools that don't recognize the key codec--or any of
this format other than varuint and varbytes--to know how many octets are in the
Multisig data object and skip over it. This format is also designed to support
any kind of digital signature, even signatures with multiple signature payloads
such as threshold signatures. This also supports building combined signatures
that contain the signed data in the <`message`> part of the signature.

## Views on the Multisig Data

To provide an abstract interface to digital signatures of all schemes and 
formats, this Multisig crate provides "views" on the Multisig data. These are 
read-only abstract interfaces to the Multisig that have implementations for 
the different supporting signature protocols.

Currently the set of views provide generic access to the "payload encoding"
codec (`multisig::AttrView`), the signature data (`multisig::SigDataView`), 
the threshold signing attributes if the protocol supports it 
(`multisig::ThresholdAttrView`) and the interface for doing threshold signature 
operations such as accessing and adding shares as well as combining shares 
(`multisig::ThresholdView`). It is important to note that the functions in the 
various views that seem to mutate the Multisig in fact do a copy-on-write (CoW)
operation and return a new Multisig with the mutation applied. This is most 
important when trying to reconstruct a threshold signature from its shares. The
best example of this is in the `multisig::Builder::try_build()` method. You'll
see that it loops over the shares adding each one and replacing it's mutable
multisig variable with the new one containing the updated shares.

```
let mut multisig = Multisig { .. };
for share in &shares {
    multisig = {
        let tv = multisig.threshold_view()?;
        // this is a CoW operation returning a mutated Multisig
        tv.add_share(share)?
    };
}
```

### What about Varsig?

There already exists a multicodec format called Varsig but it has some serious
deficiencies in design. Here is the Varsig ["spec"](https://github.com/ChainAgnostic/varsig).

This new Multisig implementation uses a new multicodec sigil `0x39` instead of 
the Varsig `0x34` to distinguish the two formats. 

The good news is that converting from Varsig to Multisig should be straight 
forward if you already have code to understand a specific Varsig format. Just
pull the relevant bits of data out of the Varsig and then use the 
`multisig::Builder` to construct a Multisig from the relevant parts.

Here's is the Varsig format as I understand it from the specification.

#### Varsig Format (May differ from the spec by the time you read this)

```
                         payload encoding
     key codec                codec
         |                      |
         v                      v
0x34 <varuint> N(<varuint>) <varuint> N(OCTET)
^                    ^                    ^
|                    |                    |
varsig      variable number of     variable number
sigil       signature specific    of signature data
                   values              octets
```

The Varsig format unfortunately has a variable number of signature-specific 
values immediately following the key codec and before the encoding codec. This
makes it impossible for a tool to decode the encoding codec when it doesn't
recognize the key codec. Since there are no counts or lengths encoded in the 
Varsig data, it is impossible to know the full length of any Varsig without
having complete support for every key codec. Multikey seeks to fix that by 
adding counts for the variable number of varuints and a length to the variable 
number of octets (i.e. [`Varbytes`](https://github.com/cryptidtech/multiutil/blob/main/src/varbytes.rs)).
