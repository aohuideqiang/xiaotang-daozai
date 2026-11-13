# 播放器跳转收尾

有些播放器的原生进度条（QQ 音乐）在跳转后会自己续播，而「暂停状态」的回报比
「播放位置」慢一拍。旧二开修过一个假失败：位置已经到位、暂停也请求过了，界面却因为
读到「还在播放」一直等到超时，把成功的跳转报成失败。

`pause_needed` 只做一件事：到位后最多再补两次暂停，之后按成功处理。

```rust
// src/smtc.rs（带 plugin: 注记的接入点）
let playing = ...;
if crate::plugins::music_seek::pause_needed(playing, pause_attempts) {
    // 再请求一次暂停
    pause_attempts += 1;
    continue;
}
return true;
```

为什么逻辑放插件、循环留核心：这段判断在 `smtc::seek` 的等待循环里，没有独立的
调用边界，把循环整个搬走反而会和上游的 `ControlWorker` 抢职责。所以只把「是否还要
再暂停」这一条规则放进插件，并带单元测试。

网易云的进度跳转不依赖这里：它走 `plugins/netease` 的播放条点击 + 日志回读。
