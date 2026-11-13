//! 播放器进度跳转的收尾判断。
//!
//! QQ 音乐的原生进度条在跳转后会自己续播，而暂停状态的回报比位置慢一拍。
//! 旧二开修过一个假失败：位置已经到位、暂停也已经请求过，却因为读到「还在播放」
//! 一直等到超时，把成功的跳转报成失败。这里把判断抽出来，核心循环只负责调用。

/// 已经确认到位后，是否还要再请求一次暂停。
///
/// `playing` 是此刻播放器回报的状态，`attempts` 是本次跳转已经补过几次暂停。
/// 最多补两次，之后按成功处理，避免暂停状态的滞后回报拖到超时。
pub(crate) fn pause_needed(playing: bool, attempts: u32) -> bool {
    playing && attempts < 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_is_retried_twice_then_treated_as_done() {
        assert!(pause_needed(true, 0));
        assert!(pause_needed(true, 1));
        assert!(!pause_needed(true, 2), "补两次后不再等待，直接按成功处理");
        assert!(!pause_needed(false, 0), "本来就没在播放，不需要暂停");
    }
}
