# LivePhoto Project Introduction

## Background

Dynamic photos are no longer a niche idea.

Apple has Live Photo. The Android ecosystem has Motion Photo. Different device vendors have also shipped their own versions of moving pictures and dynamic shots. They all aim to preserve more than a single still frame by combining a photo with a short motion segment, and sometimes audio, so that a captured moment feels more alive.

But in practice, this capability is still heavily tied to platform-specific ecosystems.

Apple Live Photo is fundamentally a paired-resource model: one still image and one MOV file connected through metadata. Android Motion Photo is closer to a single-file approach, but implementations still vary across vendors in container structure, metadata, and playback behavior. For developers, dynamic photos have long lacked a clear, stable, cross-platform container layer.

The `livephoto` project is proposed in that context.

Its goal is not to reproduce Apple's internal format, and not to bind itself to any one ecosystem. Its goal is to define an open, single-file, cross-platform dynamic photo container, and provide the tooling needed to parse, generate, and play it.

## What This Project Solves

There are several practical problems in today's dynamic photo landscape.

### 1. Native dynamic photos are often inconvenient to store and transport as a single object

Apple Live Photo is the clearest example. In practice it is not one file, but:

- one still image
- one short video
- metadata rules that bind them together

That works well inside the system photo library, but once the asset enters an application or backend system, it creates a number of engineering problems:

- uploads must handle multiple files as one logical asset
- storage systems must preserve pairing and prevent detached resources
- databases need extra indexing and association logic
- CDN distribution must move multiple resources together
- export, backup, sync, and migration can easily break the pairing

As a result, dynamic photos are often not treated as first-class media objects.

### 2. The ecosystem is fragmented and hard to reuse across platforms

Apple, Google, Samsung, Huawei, OPPO, Xiaomi, and others have all shipped dynamic photo features, but their resource organization, metadata models, playback semantics, and import/export paths are not uniform.

That leads to real development friction:

- frontend code cannot use one parsing model for every dynamic photo source
- backend systems cannot build one clean processing pipeline
- iOS, Android, Web, and desktop clients often need separate integration logic
- dynamic photos are reduced to ad hoc "image + video" business objects instead of a formal media type

### 3. Playback is often tied to vendor frameworks

Many native dynamic photo solutions feel polished inside the original platform ecosystem, but are difficult to reuse outside it.

For example:

- the Web cannot directly render Apple Live Photo as a native object
- cross-platform apps cannot depend on `PHLivePhotoView`
- backend systems cannot directly treat platform-private assets as one standard media object

That makes it difficult to productize dynamic photos as a truly cross-platform capability.

### 4. Playback semantics are rarely unified

A dynamic photo is not just "an image and a video stored together".

It also needs semantic rules for:

- what should be shown by default
- which video moment corresponds to the cover still
- whether playback should autoplay
- whether playback should loop
- whether audio should be muted by default
- whether the player should return to the cover image or hold the last frame

If these semantics are not defined at the format level, every player, editor, and exporter is forced to guess, and the user experience becomes inconsistent across platforms.

## The `livephoto` Solution

The `livephoto` project addresses this by defining a single-file binary container format, `.livephoto`, that can encapsulate:

- a primary cover image
- a primary motion video
- a manifest describing structure and semantics
- optional resources such as thumbnails, EXIF, XMP, hashes, and bridge metadata

On top of that container, the project builds:

- a Rust format implementation
- CLI tools for packing and inspection
- a Web player
- bridge layers for Apple Live Photo and Android Motion Photo workflows

In other words, this project is not another camera feature. It is an attempt to provide a proper infrastructure layer for dynamic photos.

## Core Value of the `.livephoto` Format

### 1. Single-file packaging

The first principle of `.livephoto` is that a dynamic photo should be one self-contained object.

That simplifies:

- object storage
- database archival
- API upload and download
- CDN caching and distribution
- message attachments
- cloud sync and backup

### 2. Cross-platform parsing and playback

The format is not tied to Apple Photos or Android system components.

Once a parser exists, the same file can be handled across:

- Web
- iOS
- Android
- macOS
- Windows
- Linux

### 3. Decoupling the container from media codecs

`.livephoto` does not redefine image or video codecs. It treats encoded media as payloads carried by the container.

That allows it to transport:

- JPEG / HEIC / HEIF / AVIF / PNG / WebP
- MP4 / MOV / WebM

This keeps media encoding separate from resource organization.

### 4. Explicit playback semantics

The format is not just a wrapper around image bytes and video bytes. The manifest defines:

- duration
- cover timestamp
- references to the primary image and video
- whether the asset has audio
- default playback behavior

That gives different implementations a shared semantic model instead of leaving everything to player-specific assumptions.

### 5. Bridgeability with existing ecosystems

`livephoto` does not reject existing dynamic photo ecosystems.

Instead, it treats Apple Live Photo and Android Motion Photo as bridge targets:

- Apple assets can be imported into `.livephoto`
- `.livephoto` assets can be exported back into Apple-compatible still+MOV pairs
- Android Motion Photo related metadata can be preserved as bridge information

This makes the format a unifying layer rather than a closed replacement.

## Why This Matters

### Product value

The project allows dynamic photos to become a first-class media type outside a single vendor's photo app.

That means:

- social apps can handle dynamic photos as first-class attachments
- gallery apps can manage dynamic photos across platforms
- Web products can present dynamic photos directly instead of degrading them to GIFs or plain video
- media platforms can unify images, videos, and dynamic photos under one model

### Engineering value

It moves dynamic photos from ad hoc business modeling into a formal format layer.

That enables:

- clearer storage models
- more stable import/export pipelines
- more reusable player interfaces
- more maintainable backend processing
- cleaner format evolution

### Ecosystem value

Today, dynamic photos are mostly a platform capability, not an open media layer.

The `livephoto` project attempts to abstract that capability into a developer-friendly, implementable, evolvable container model. That is useful for open tooling, SDKs, editors, and transcoders.

## Use Cases

This project is suitable for scenarios such as:

- dynamic photo management in gallery applications
- dynamic photo attachments in social products
- cloud media archival systems
- media asset management platforms
- Web-based dynamic photo presentation
- cross-platform players
- import/export and migration tools
- dynamic photo editors

If a system wants to preserve both the still-photo identity and the short motion context of an asset, `.livephoto` is likely a relevant fit.

## Design Principles

### 1. Single-file first

A dynamic photo should be one object, not a weak association between multiple files.

### 2. Define the format first, then build the tooling around it

Players, editors, and converters should be built on top of a stable container and metadata model, not the other way around.

### 3. Keep semantics explicit

The cover still, motion clip, timing, and playback policy should be clearly defined at the format level.

### 4. Stay extensible, but keep v1 focused

The first version should solve the core problems:

- file organization
- primary resource references
- metadata semantics
- bridge metadata

It should not take on unnecessary complexity too early.

### 5. Decouple from any single vendor ecosystem

The project can bridge Apple and Android workflows, but it does not treat one vendor's private representation as the format itself.

## Project Components

The project currently includes:

- format specification documents
- the Rust implementation crate `livephoto-format`
- the CLI tool `livephoto-cli`
- a minimal Web player

It can later grow into:

- an iOS SDK
- an Android SDK
- an editor
- a transcoder
- backend processing components

## Relationship to Apple Live Photo

This point needs to be explicit:

`livephoto` is not Apple's native Live Photo format.

Its relationship to Apple Live Photo is:

- it can import Apple-originated assets
- it can preserve Apple bridge metadata
- it can export back into Apple-compatible still+MOV resources
- it is not expected to be natively recognized by Apple Photos as a Live Photo file

So the goal is not to impersonate Apple's private format. The goal is to provide a more general container layer that is easier to use in real systems.

## Project Boundaries

`livephoto` currently addresses the container-layer problem for dynamic photos. It does not attempt to solve every media problem in v1.

It does not aim to directly solve:

- custom video codecs
- DRM
- native system photo library integration
- advanced editing timelines
- complex streaming protocols

Those capabilities can be built later in adjacent tools, but they should not compromise the clarity of the format layer.

## Long-Term Direction

The long-term goals of the project include:

- becoming a cross-platform dynamic image container
- serving as a bridge layer between Apple Live Photo and Android Motion Photo ecosystems
- providing stable parsers, generators, players, and editors
- giving dynamic photos a consistent handling model across Web, mobile, and backend systems

More broadly, the project is not only about "how to play a moving photo". It is about creating a more open and unified infrastructure layer for this category of media.

## Summary

The `livephoto` project is trying to solve a long-standing problem that still lacks a clean answer:

How do you organize "a still image + a short motion clip + playback semantics + bridge metadata" into a formal media object that is cross-platform, storable, transferable, playable, and evolvable?

The answer proposed by `.livephoto` is:

use an open, single-file container to elevate dynamic photos from a platform feature into a format capability, and then build tooling around that format.

That improves storage and transport, but more importantly, it allows dynamic photos to move beyond a single vendor ecosystem and become a more general cross-platform media type.
