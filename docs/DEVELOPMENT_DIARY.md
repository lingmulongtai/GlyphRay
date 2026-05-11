# GlyphRay Development Diary

## 2026-05-11 JST - Day 1, Milestone 1 Foundation

今日は GlyphRay が、ただの構想から実際のリポジトリになった日。モノレポ構造、Rust の共有 crate、Android アプリの入口、Windows host、macOS shell、protocol 定義、最初のテスト、そして product/security/architecture docs を一気に敷いた。

いちばん大事な背骨は stylus path。Android 側では raw `MotionEvent` を見られるようになり、Windows 側には `CreateSyntheticPointerDevice` / `InjectSyntheticPointerInput` を使う native synthetic pen injection wrapper が入った。まだ端から端までつながってはいないけれど、GlyphRay が何者になるかはコードの形として見え始めた。

この時点の進捗見積もり: 20%。

## 2026-05-11 JST - Day 1, Milestone 2 First Pass

今日は Milestone 2 に踏み込んだ。Android には low-latency `SurfaceView` と `MediaCodec` H.264 decoder wrapper を追加し、remote session 画面には latency overlay を載せた。Windows host には monitor enumeration、low-latency H.264 encoder abstraction、そして後続の hardware/software backend を差し込むための境界を追加した。

transport 側では UDP datagram packet format を作った。channel、message kind、sequence、timestamp、payload length、checksum を持つ小さな層で、input/control packet にはもう使える形になっている。さらに video frame の chunking/reassembly utility も入れたので、大きい H.264 frame を UDP packet に分ける準備まで進んだ。

README も整えて、開発進捗率を毎回更新する場所を作った。正直な現在地は、Milestone 1 は完了、Milestone 2 は映像と transport の配管が組み上がり始めたところ。次の山は Windows capture と、実際の H.264 frame を LAN で Android decoder に流し込むところ。

この時点の進捗見積もり: 31%。
