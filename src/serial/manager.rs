use serialport::*;

pub struct SerialManager{
    id: u32
}

impl SerialManager{
    fn get_ports(self) -> Vec<String> {
        let mut ports = vec![];
        for port in serialport::available_ports().unwrap() {
            ports.push(port.port_name);
        }
        ports
    }

    pub fn connect_port(&mut self) {
        match serialport::new(&port_name, 9600)
            .timeout(Duration::from_millis(100))
            .flow_control(serialport::FlowControl::None)
            .open() {
            Ok(mut p) => {
                let _write_data_terminal_ready = p.write_data_terminal_ready(true);
                *port_guard = Some(p);
            }
            Err(e) => {
                println!("Error conectando: {}", e);
            },
        }
    }
}