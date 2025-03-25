mod virtual_controller;

use std::ops::DerefMut;
use std::net::TcpStream;
use std::time::{Duration, Instant};
use std::io::ErrorKind;

use godot::{classes::Button, prelude::*};
use virtual_controller::VirtualController;

struct FRCInterface;

#[gdextension]
unsafe impl ExtensionLibrary for FRCInterface {}

#[derive(GodotClass)]
#[class(base=Node3D)]
struct FRCInterfaceBase {
    #[export]
    connected: bool,

    force_connected: bool,

    #[export]
    climb_button: Option<Gd<Button>>,
    
    #[export]
    zero_button: Option<Gd<Button>>,
    
    #[export]
    intake_button: Option<Gd<Button>>,
    
    #[export]
    high_button: Option<Gd<Button>>,
    
    #[export]
    mid_button: Option<Gd<Button>>,
    
    #[export]
    low_button: Option<Gd<Button>>,

    #[export]
    coral_button: Option<Gd<Button>>,
    
    #[export]
    intake_alga_button: Option<Gd<Button>>,
    
    #[export]
    drop_alga_button: Option<Gd<Button>>,

    virtual_controller: Option<VirtualController>,
    
    // TCP ping fields
    last_ping_time: Instant,
    ping_interval: Duration,

    #[export]
    ping_address: GString,

    #[export]
    ping_port: i64,
    
    // Add the base field
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for FRCInterfaceBase {
    fn init(base: Base<Self::Base>) -> Self {
        FRCInterfaceBase {
            connected: false,
            force_connected: false,
            climb_button: None,
            zero_button: None,
            intake_button: None,
            high_button: None,
            mid_button: None,
            low_button: None,
            coral_button: None,
            intake_alga_button: None,
            drop_alga_button: None,
            virtual_controller: None,
            last_ping_time: Instant::now(),
            ping_interval: Duration::from_secs(15),
            ping_address: "10.45.33.2".into(),
            ping_port: 22,
            base,
        }
    }

    fn ready(&mut self) {
        // Connect button signals
        self.connect_button_signals();
        
        // Initialize the virtual controller
        let mut controller = VirtualController::new();
        if controller.initialize() {
            godot_print!("Virtual controller initialized");
            self.virtual_controller = Some(controller);
        } else {
            godot_error!("Failed to initialize virtual controller");
        }
        
        // Perform initial ping
        self.ping_tcp_server();
    }

    fn process(&mut self, _delta: f64) {
        // Check if it's time to ping again
        if self.last_ping_time.elapsed() >= self.ping_interval {
            self.ping_tcp_server();
            self.last_ping_time = Instant::now();
        }
    }
    
    fn exit_tree(&mut self) {
        // Shutdown the virtual controller
        if let Some(mut controller) = self.virtual_controller.take() {
            controller.shutdown();
        }
    }
}

#[godot_api]
impl FRCInterfaceBase {
    fn connect_button_signals(&mut self) {
        // Helper to connect button signals
        let connect_button = |button: &Option<Gd<Button>>, name: &str, base_obj: &Gd<Node3D>| {
            if let Some(gd_btn) = button {
                // Get a mutable reference by cloning and using bind_mut
                let mut btn = gd_btn.clone();
                let btn_mut = btn.deref_mut();
                
                // Create the StringName for the button name once
                let button_name = StringName::from(name);
                let name_variant = button_name.to_variant();
                
                // Connect button_down signal
                let callable_pressed = Callable::from_object_method(base_obj, "on_button_pressed");
                let bound_callable_pressed = callable_pressed.bind(&[name_variant.clone()]);
                
                let result = btn_mut.connect("button_down", &bound_callable_pressed);
                if result != godot::global::Error::OK {
                    godot_error!("Failed to connect button_down for {}: {:?}", name, result);
                }
                
                // Connect button_up signal
                let callable_released = Callable::from_object_method(base_obj, "on_button_released");
                let bound_callable_released = callable_released.bind(&[name_variant]);
                
                let result = btn_mut.connect("button_up", &bound_callable_released);
                if result != godot::global::Error::OK {
                    godot_error!("Failed to connect button_up for {}: {:?}", name, result);
                }
            }
        };
        
        // Get a reference to this node as a Gd<Node3D>
        let base = self.base();
        
        // Connect all buttons
        connect_button(&self.climb_button, "climb", &base);
        connect_button(&self.zero_button, "zero", &base);
        connect_button(&self.intake_button, "intake", &base);
        connect_button(&self.high_button, "high", &base);
        connect_button(&self.mid_button, "mid", &base);
        connect_button(&self.low_button, "low", &base);
        connect_button(&self.coral_button, "coral", &base);
        connect_button(&self.intake_alga_button, "intake_alga", &base);
        connect_button(&self.drop_alga_button, "drop_alga", &base);
    }
    
    fn ping_tcp_server(&mut self) {
        // Try to connect to the TCP server
        if self.force_connected {
            self.connected = true;
            return;
        }

        match TcpStream::connect_timeout(
            &format!("{}:{}", self.ping_address, self.ping_port)
                .parse()
                .unwrap_or_else(|_| {
                    godot_error!("Invalid address format");
                    std::net::SocketAddr::from(([127, 0, 0, 1], 22))
                }),
            Duration::from_secs(2),
        ) {
            Ok(_) => {
                if !self.connected {
                    godot_print!("TCP connection established with {}:{}", self.ping_address, self.ping_port);
                    self.connected = true;
                }
            }
            Err(e) => {
                if self.connected {
                    match e.kind() {
                        ErrorKind::TimedOut => {
                            godot_warn!("TCP connection timed out with {}:{}", self.ping_address, self.ping_port);
                        }
                        ErrorKind::ConnectionRefused => {
                            godot_warn!("TCP connection refused by {}:{}", self.ping_address, self.ping_port);
                        }
                        _ => {
                            godot_warn!("TCP connection error with {}:{}: {}", self.ping_address, self.ping_port, e);
                        }
                    }
                    self.connected = false;
                }
            }
        }
    }
    
    #[func]
    fn on_button_pressed(&mut self, button_name: StringName) {
        if !self.connected {
            godot_warn!("Not connected, cannot send button press");
            return;
        }
        
        if let Some(controller) = &self.virtual_controller {
            controller.set_button(&button_name.to_string(), true);
        }
    }
    
    #[func]
    fn on_button_released(&mut self, button_name: StringName) {
        if !self.connected {
            return;
        }
        
        if let Some(controller) = &self.virtual_controller {
            controller.set_button(&button_name.to_string(), false);
        }
    }

    #[func]
    fn toggle_force_connected(&mut self) {
        self.force_connected = !self.force_connected;
        if self.force_connected {
            self.connected = true;
        } else {
            self.ping_tcp_server();
        }
    }
}