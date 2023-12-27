use std::io::Read;
use tun::Device;

fn main() {
    let mut config = tun::Configuration::default();
    config.address((10, 0, 0, 1))
        .netmask((255, 255, 255, 0))
        .up();

    #[cfg(target_os = "linux")]
    config.platform(|config| {
        config.packet_information(true);
    });

    let mut dev = tun::create(&config).unwrap();
    // dev.set_destination("192.168.....")
    let mut buf = [0; 4096];

    loop {
        let amount = dev.read(&mut buf).unwrap();
        println!("{:?}", &buf[0 .. amount]);
    }
}
