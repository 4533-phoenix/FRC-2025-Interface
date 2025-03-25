use godot::prelude::*;
use vigem_client::XButtons;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::sync::atomic::Ordering; // Import Ordering directly

pub struct VirtualController {
    client: Option<vigem_client::Client>,
    target: Option<Arc<Mutex<vigem_client::XTarget>>>,
    control_thread: Option<thread::JoinHandle<()>>,
    running: Arc<std::sync::atomic::AtomicBool>,
    button_state: Arc<Mutex<ButtonState>>,
}

#[derive(Default)]
struct ButtonState {
    climb: bool,
    zero: bool,
    intake: bool,
    high: bool,
    mid: bool,
    low: bool,
    coral: bool,
    intake_alga: bool,
    drop_alga: bool,
}

impl VirtualController {
    pub fn new() -> Self {
        Self {
            client: None,
            target: None,
            control_thread: None,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            button_state: Arc::new(Mutex::new(ButtonState::default())),
        }
    }

    pub fn initialize(&mut self) -> bool {
        // Try to connect to the ViGEm client
        match vigem_client::Client::connect() {
            Ok(client) => {
                // Create the XTarget (Xbox controller)
                let mut target = vigem_client::XTarget::new(client, vigem_client::TargetId::XBOX360_WIRED);
                
                // Plugin the virtual controller
                if let Err(e) = target.plugin() {
                    godot_error!("Failed to plugin virtual controller: {}", e);
                    return false;
                }
                
                // Wait for the controller to be ready
                if let Err(e) = target.wait_ready() {
                    godot_error!("Failed to wait for virtual controller ready: {}", e);
                    return false;
                }
                
                // Store the client and target
                let target = Arc::new(Mutex::new(target));
                self.target = Some(target.clone());
                
                // Start the control thread
                self.running.store(true, Ordering::SeqCst); // Fixed ordering
                let running = self.running.clone();
                let button_state = self.button_state.clone();
                
                self.control_thread = Some(thread::spawn(move || {
                    let mut last_state = ButtonState::default();
                    
                    while running.load(Ordering::SeqCst) { // Fixed ordering
                        // Lock the button state
                        let current_state = {
                            let guard = button_state.lock().unwrap();
                            ButtonState {
                                climb: guard.climb,
                                zero: guard.zero,
                                intake: guard.intake,
                                high: guard.high,
                                mid: guard.mid,
                                low: guard.low,
                                coral: guard.coral,
                                intake_alga: guard.intake_alga,
                                drop_alga: guard.drop_alga,
                            }
                        };
                        
                        // Check if the state changed
                        if current_state.climb != last_state.climb
                           || current_state.zero != last_state.zero
                           || current_state.intake != last_state.intake
                           || current_state.high != last_state.high
                           || current_state.mid != last_state.mid
                           || current_state.low != last_state.low
                           || current_state.coral != last_state.coral
                           || current_state.intake_alga != last_state.intake_alga
                           || current_state.drop_alga != last_state.drop_alga {
                            
                            // Update the controller state
                            // Create a new buttons object for each button press
                            let mut button_value = 0u16;
                            
                            // Use the constants correctly
                            if current_state.climb {
                                button_value |= XButtons::START;
                            }
                            if current_state.zero {
                                button_value |= XButtons::BACK;
                            }
                            if current_state.intake {
                                button_value |= XButtons::RIGHT;
                            }
                            if current_state.high {
                                button_value |= XButtons::UP;
                            }
                            if current_state.mid {
                                button_value |= XButtons::LEFT;
                            }
                            if current_state.low {
                                button_value |= XButtons::DOWN;
                            }
                            if current_state.coral {
                                button_value |= XButtons::B;
                            }
                            if current_state.intake_alga {
                                button_value |= XButtons::LB;
                            }
                            if current_state.drop_alga {
                                button_value |= XButtons::RB;
                            }
                            
                            let buttons = vigem_client::XButtons(button_value);
                            
                            let gamepad = vigem_client::XGamepad {
                                buttons,
                                ..Default::default()
                            };
                            
                            if let Ok(mut t) = target.lock() {
                                if let Err(e) = t.update(&gamepad) {
                                    godot_error!("Failed to update virtual controller: {}", e);
                                }
                            }
                            
                            last_state = current_state;
                        }
                        
                        // Sleep for a short time
                        thread::sleep(Duration::from_millis(10));
                    }
                }));
                
                return true;
            }
            Err(e) => {
                godot_error!("Failed to connect to ViGEm client: {}", e);
                return false;
            }
        }
    }
    
    pub fn shutdown(&mut self) {
        if self.running.load(Ordering::SeqCst) {
            self.running.store(false, Ordering::SeqCst);
            
            if let Some(handle) = self.control_thread.take() {
                let _ = handle.join();
            }
            
            self.target = None;
            self.client = None;
        }
    }
    
    pub fn set_button(&self, button: &str, pressed: bool) {
        if let Ok(mut state) = self.button_state.lock() {
            match button {
                "climb" => state.climb = pressed,
                "zero" => state.zero = pressed,
                "intake" => state.intake = pressed,
                "high" => state.high = pressed,
                "mid" => state.mid = pressed,
                "low" => state.low = pressed,
                "coral" => state.coral = pressed,
                "intake_alga" => state.intake_alga = pressed,
                "drop_alga" => state.drop_alga = pressed,
                _ => godot_warn!("Unknown button: {}", button),
            }
        }
    }
}

impl Drop for VirtualController {
    fn drop(&mut self) {
        self.shutdown();
    }
}
