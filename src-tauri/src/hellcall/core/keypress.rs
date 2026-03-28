use anyhow::{anyhow, Result};
use log::{info, trace};
use rdev::{simulate, Button, EventType, Key};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
    sync::mpsc,
    sync::{Arc, Mutex, RwLock},
    thread::JoinHandle,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Input {
    Button(Button), // 让 Unknown 优先绑定到Button
    Key(Key),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LocalKey {
    UP,
    DOWN,
    LEFT,
    RIGHT,
    /// 打开战备页面按键
    OPEN,
    /// 扔出战备, 一般是鼠标左键
    THROW,
    /// 重新执行上一次键盘宏按键
    RESEND,
    /// Push-to-Talk 按住说话
    PTT,
    /// OCC (One-Click Completion) 触发视觉截图与自动拨号
    OCC,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyPresserConfig {
    /// 等待打开战备页面的时间
    pub wait_open_time: u64,
    /// 按键释放间隔
    pub key_release_interval: u64,
    /// 按键间隔
    pub diff_key_interval: u64,
}

impl Default for KeyPresserConfig {
    fn default() -> Self {
        Self {
            wait_open_time: 30,
            key_release_interval: 30,
            diff_key_interval: 20,
        }
    }
}

pub struct KeyPresser {
    config: Arc<RwLock<KeyPresserConfig>>,
    /// 按键映射
    key_map: Arc<RwLock<HashMap<LocalKey, Input>>>,
    shortcut: Arc<RwLock<HashMap<Input, Vec<LocalKey>>>>,
    one_stack: Arc<Mutex<Option<Vec<LocalKey>>>>,
    spare_stack: Arc<Mutex<Option<Vec<LocalKey>>>>,
    tx: Option<mpsc::Sender<(Vec<LocalKey>, bool)>>,
    worker_handle: Option<JoinHandle<()>>,
    /// 当前正在模拟按键的数量，用于 listen 回调过滤注入事件
    simulating: Arc<AtomicUsize>,
    listen_key_map: Arc<
        Mutex<
            HashMap<
                Input,
                Box<
                    dyn FnMut(bool, Box<dyn Fn(Vec<LocalKey>, bool) + Send + 'static>)
                        + Send
                        + 'static,
                >,
            >,
        >,
    >,
}

impl KeyPresser {
    fn check_key_map(key_map: &HashMap<LocalKey, Input>) -> Result<()> {
        for local_key in [
            LocalKey::UP,
            LocalKey::DOWN,
            LocalKey::LEFT,
            LocalKey::RIGHT,
            LocalKey::OPEN,
            LocalKey::THROW,
        ] {
            if !key_map.contains_key(&local_key) {
                return Err(anyhow!("Missing mapping for LocalKey: {:?}", local_key));
            }
        }
        Ok(())
    }

    pub fn update_config(
        &self,
        config: KeyPresserConfig,
        key_map: HashMap<LocalKey, Input>,
        shortcut: HashMap<Input, Vec<LocalKey>>,
    ) -> Result<()> {
        Self::check_key_map(&key_map)?;
        *self.config.write().unwrap() = config;
        *self.key_map.write().unwrap() = key_map;
        *self.shortcut.write().unwrap() = shortcut;
        Ok(())
    }

    pub fn new(
        config: KeyPresserConfig,
        key_map: HashMap<LocalKey, Input>,
        shortcut: HashMap<Input, Vec<LocalKey>>,
    ) -> Result<Self> {
        Self::check_key_map(&key_map)?;

        // keypress worker
        let (tx, rx) = std::sync::mpsc::channel::<(Vec<LocalKey>, bool)>();
        let config = Arc::new(RwLock::new(config));
        let key_map = Arc::new(RwLock::new(key_map));
        let simulating = Arc::new(AtomicUsize::new(0));
        let handle = std::thread::spawn({
            let config = Arc::clone(&config);
            let key_map = Arc::clone(&key_map);
            let simulating = Arc::clone(&simulating);
            move || {
                // 定义一个模拟按键的闭包，自动维护 simulating 计数，确保 listen 回调能正确过滤注入事件
                let sim = |event: &EventType| {
                    if let Err(e) = simulate(event) {
                        log::error!("simulate error: {:?}", e);
                    }
                };

                while let Ok((keys, fast)) = rx.recv() {
                    info!("key pressed: {:?}, fast: {}", keys, fast);

                    simulating.fetch_add(1, Ordering::Relaxed);

                    let c = config.read().unwrap();
                    let (wait_open_time, key_release_interval, diff_key_interval) = {
                        (
                            c.wait_open_time.clone(),
                            c.key_release_interval.clone(),
                            c.diff_key_interval.clone(),
                        )
                    };
                    drop(c);

                    // convert keys to events
                    let km = key_map.read().unwrap();
                    let key_event_map: Vec<(LocalKey, EventType, EventType)> = keys
                        .iter()
                        .map(|k| {
                            let local_key = k.clone();
                            let input = km.get(k).unwrap().clone();
                            let (press_event_type, release_event_type) = match input {
                                Input::Key(key) => {
                                    (EventType::KeyPress(key), EventType::KeyRelease(key))
                                }
                                Input::Button(button) => (
                                    EventType::ButtonPress(button),
                                    EventType::ButtonRelease(button),
                                ),
                            };
                            (local_key, press_event_type, release_event_type)
                        })
                        .collect();
                    drop(km);

                    // simulating
                    let mut open_release_event: Option<EventType> = None;
                    let mut is_waited_open = fast;
                    for (key, press_event_type, release_event_type) in &key_event_map {
                        if key == &LocalKey::OPEN {
                            sim(press_event_type);
                            trace!("simulated press [OPEN] event: {:?}", press_event_type);
                            open_release_event = Some(release_event_type.clone());
                            is_waited_open = false;
                            continue;
                        } else if !is_waited_open {
                            std::thread::sleep(Duration::from_millis(wait_open_time));
                            is_waited_open = true;
                        }

                        sim(press_event_type);
                        trace!("simulated press event: {:?}", press_event_type);

                        std::thread::sleep(Duration::from_millis(key_release_interval));
                        sim(release_event_type);
                        trace!("simulated release event: {:?}", release_event_type);
                        std::thread::sleep(Duration::from_millis(diff_key_interval));
                    }
                    if let Some(event) = open_release_event {
                        sim(&event);
                        trace!("simulated release [OPEN] event: {:?}", event);
                    }

                    simulating.fetch_sub(1, Ordering::Relaxed);
                }
            }
        });

        Ok(Self {
            config,
            key_map,
            shortcut: Arc::new(RwLock::new(shortcut)),
            one_stack: Arc::new(Mutex::new(None)),
            spare_stack: Arc::new(Mutex::new(None)),
            tx: Some(tx),
            worker_handle: Some(handle),
            simulating,
            listen_key_map: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn push(&self, keys: &[LocalKey]) {
        let keys = keys.to_vec();

        if let Some(first_key) = keys.first() {
            if first_key == &LocalKey::OPEN {
                if let Some(tx) = &self.tx {
                    if let Err(e) = tx.send((keys.clone(), false)) {
                        log::error!("push send error: {:?}", e);
                    }
                }
            } else {
                *self.one_stack.lock().unwrap() = Some(keys.clone());
                *self.spare_stack.lock().unwrap() = Some(keys.clone());
            }
        }
    }

    /// 注册一个全局按键监听器
    ///
    /// 当指定的按键 `key` 被按下或释放时，会触发 `callback` 函数。
    /// `callback` 的参数为 `true` 表示按下，`false` 表示释放。
    pub fn listen_key<F>(&self, key: Input, callback: F)
    where
        F: FnMut(bool, Box<dyn Fn(Vec<LocalKey>, bool) + Send + 'static>) + Send + 'static,
    {
        self.listen_key_map
            .lock()
            .unwrap()
            .insert(key, Box::new(callback));
    }

    pub fn clear_listen_map(&self) {
        self.listen_key_map.lock().unwrap().clear();
    }

    /// block
    pub fn listen(&self) -> Result<()> {
        let shortcut = Arc::clone(&self.shortcut);
        let key_map = Arc::clone(&self.key_map);
        let one_stack = Arc::clone(&self.one_stack);
        let spare_stack = Arc::clone(&self.spare_stack);
        let tx = self.tx.as_ref().unwrap().clone();
        let simulating = Arc::clone(&self.simulating);
        let listen_key_map = Arc::clone(&self.listen_key_map);

        // block
        rdev::listen(move |event| {
            // 首先处理注册的监听按键（包括按下和松开），不受 simulate 状态影响
            {
                // 在加锁前，提取出目标 Input 以及是否为按下状态 (is_press)
                let event_info = match &event.event_type {
                    EventType::KeyPress(k) => Some((Input::Key(k.clone()), true)),
                    EventType::KeyRelease(k) => Some((Input::Key(k.clone()), false)),
                    EventType::ButtonPress(b) => Some((Input::Button(b.clone()), true)),
                    EventType::ButtonRelease(b) => Some((Input::Button(b.clone()), false)),
                    _ => None,
                };
                // 如果属于我们关心的事件类型，再去尝试获取锁
                if let Some((target_input, is_press)) = event_info {
                    if let Ok(mut listeners) = listen_key_map.try_lock() {
                        if let Some(callback) = listeners.get_mut(&target_input) {
                            let tx_clone = tx.clone();
                            let fn_push: Box<dyn Fn(Vec<LocalKey>, bool) + Send + 'static> =
                                Box::new(move |keys, fast| {
                                    let _ = tx_clone.send((keys, fast));
                                });

                            callback(is_press, fn_push);
                        }
                    }
                }
            }

            // 忽略由 simulate 注入的事件，防止模拟按键误触发快捷键循环
            if simulating.load(Ordering::Relaxed) > 0 {
                return;
            }

            // 只处理按下事件，其余直接返回，保持钩子回调轻量
            let Some(input) = (match event.event_type {
                EventType::KeyPress(key) => Some(Input::Key(key)),
                EventType::ButtonPress(key) => Some(Input::Button(key)),
                _ => None,
            }) else {
                return;
            };

            let (open_key, resend_key) = {
                let Ok(km) = key_map.try_read() else {
                    return;
                };
                (
                    km.get(&LocalKey::OPEN).unwrap().clone(),
                    km.get(&LocalKey::RESEND).unwrap().clone(),
                )
            };

            if input == open_key {
                // 使用 try_lock 非阻塞：若锁被占用则跳过，绝不阻塞系统钩子
                if let Ok(mut guard) = one_stack.try_lock() {
                    if let Some(keys) = guard.take() {
                        if let Err(e) = tx.send((keys, false)) {
                            log::error!("listen send error: {:?}", e);
                        }
                    }
                }
            } else if input == resend_key {
                // 先读 spare_stack，再写 one_stack，避免同时持有两把锁（防死锁）
                let keys_opt = spare_stack.try_lock().ok().and_then(|g| g.clone());
                if let Some(keys) = keys_opt {
                    info!("resend key press: {:?}", &keys);
                    if let Ok(mut guard) = one_stack.try_lock() {
                        guard.replace(keys);
                    }
                }
            } else {
                let Ok(sc) = shortcut.try_read() else {
                    return;
                };
                if let Some(keys) = sc.get(&input).cloned() {
                    drop(sc);
                    if let Err(e) = tx.send((keys, true)) {
                        log::error!("shortcut send error: {:?}", e);
                    }
                }
            }
        })
        .map_err(|err| anyhow!("listen key press error: {:?}", err))?;

        Ok(())
    }

    pub fn has_validity(keys: &[LocalKey]) -> Result<()> {
        if keys.is_empty() {
            return Err(anyhow!("keys must not be empty"));
        }

        if keys.contains(&LocalKey::RESEND) {
            return Err(anyhow!("cannot use RESEND key in a macro"));
        }

        let has_open = keys.contains(&LocalKey::OPEN);
        let has_throw = keys.contains(&LocalKey::THROW);

        // OPEN 必须在第一位
        if has_open && keys.first() != Some(&LocalKey::OPEN) {
            return Err(anyhow!("OPEN key must be the first key"));
        }

        // THROW 必须在最后一位
        if has_throw && keys.last() != Some(&LocalKey::THROW) {
            return Err(anyhow!("THROW key must be the last key"));
        }

        // 中间段不允许再出现 OPEN 或 THROW
        if keys.len() > 2 {
            for key in &keys[1..keys.len() - 1] {
                if key == &LocalKey::OPEN {
                    return Err(anyhow!("OPEN key must be the first key"));
                }
                if key == &LocalKey::THROW {
                    return Err(anyhow!("THROW key must be the last key"));
                }
            }
        }

        Ok(())
    }
}

impl Drop for KeyPresser {
    fn drop(&mut self) {
        // Drop tx first to close the channel: the worker thread's rx.recv()
        // will return Err and the while-loop exits naturally.
        drop(self.tx.take());
        // Then join to wait for the worker thread to fully exit.
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}
