use anyhow;
use embedded_svc::mqtt::client::Event;
use embedded_svc::mqtt::client::QoS;
use embedded_svc::wifi::{AuthMethod, ClientConfiguration, Configuration};
use embedded_svc::mqtt::client::Connection;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::mqtt::client::{EspMqttClient, MqttClientConfiguration};
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use std::{thread::sleep, time::Duration};

// Define Wifi SSID and Password
const WIFI_SSID: &str = "";
const WIFI_PASSWORD: &str = "";
const MQTT_BROKER_URL: &str = "mqtt://6804c12a8e254e5c9f0d45b0ea9c0b2a.s1.eu.hivemq.cloud";
const MQTT_PORT: u16 = 8883;
const MQTT_TOPIC: &str = "testtopic/1";
const MQTT_CLIENT_ID: &str = "esp32";
const MQTT_USERNAME: &str = "hivemq.webclient.1726606709307";
const MQTT_PASSWORD: &str = "9wD6c7YmrP<tIi!5VL#>";

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. 
    // Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. 
    // See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_sys::link_patches();

    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    /// Initializes a mutable `wifi` instance by wrapping an `EspWifi` object with `BlockingWifi`.
    /// 
    /// The `EspWifi` object is created using the provided modem peripheral, system event loop, 
    /// and an optional non-volatile storage (NVS) reference. The `BlockingWifi::wrap` function 
    /// is then used to create a blocking Wi-Fi instance, which is assigned to `wifi`.
    ///
    /// # Parameters
    /// - `peripherals.modem`: The modem peripheral used for Wi-Fi.
    /// - `sysloop`: The system event loop used for handling Wi-Fi events.
    /// - `nvs`: An optional reference to non-volatile storage (NVS).
    ///
    /// # Returns
    /// A result containing the initialized `wifi` instance or an error if the initialization fails.
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sysloop.clone(), Some(nvs))?,
        sysloop,
    )?;

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: self::WIFI_SSID.into(),
        bssid: None,
        auth_method: AuthMethod::None,
        password: self::WIFI_PASSWORD.into(),
        channel: None,
    }))?;

    // Start Wifi
    wifi.start()?;

    // Connect Wifi
    wifi.connect()?;

    // Wait until the network interface is up
    wifi.wait_netif_up()?;

    // Print Out Wifi Connection Configuration
    while !wifi.is_connected().unwrap() {
        // Get and print connection configuration
        let config = wifi.get_configuration().unwrap();
        println!("Waiting for station {:?}", config);
    }

    println!("Wifi Connected");

    // Create MQTT Client Configuration
    let mqtt_config = MqttClientConfiguration {
        broker_url: MQTT_BROKER_URL.into(),
        port: MQTT_PORT,
        client_id: MQTT_CLIENT_ID.into(),
        username: Some(MQTT_USERNAME.into()),
        password: Some(MQTT_PASSWORD.into()),
        keep_alive: 60,
        clean_session: true,
        lwt_topic: None,
        lwt_message: None,
        lwt_qos: QoS::AtLeastOnce,
        lwt_retain: false,
    };

    // Create MQTT Client
    let mut client = EspMqttClient::new(mqtt_config, sysloop.clone())?;

    // Subscribe to MQTT Topic
    client.subscribe(MQTT_TOPIC.into(), QoS::AtLeastOnce)?;

    // Publish to MQTT Topic
    client.publish(MQTT_TOPIC.into(), QoS::AtLeastOnce, false, "Hello World".as_bytes())?;

    // Event Loop
    sysloop.run(|event| {
        match event {
            Event::Mqtt(client_event) => {
                match client_event {
                    Connection::Connected => {
                        println!("Connected to MQTT Broker");
                    }
                    Connection::Disconnected => {
                        println!("Disconnected from MQTT Broker");
                    }
                    Connection::MessageReceived(topic, message) => {
                        println!("Received message on topic: {:?} with message: {:?}", topic, message);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    })?;

    loop {
        // Keep waking up device to avoid watchdog reset
        sleep(Duration::from_millis(1000));
    }
}
