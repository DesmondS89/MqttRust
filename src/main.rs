use anyhow;

use embedded_svc::{
    http::{Headers, Method},
    io::{Read, Write},
    wifi::{self, AuthMethod, ClientConfiguration, Configuration},
};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::http::server::EspHttpServer;
use esp_idf_svc::mqtt::client::{EspMqttClient, MqttClientConfiguration};
use esp_idf_svc::mqtt::client::{EspMqttConnection, QoS};
use embedded_svc::mqtt::client::Event;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use heapless::String;
use log::*;
use serde::{Deserialize, Serialize};
use serde_urlencoded;
use std::str::FromStr;
use std::{thread::sleep, time::Duration};
use esp_idf_svc::mqtt::client::EspMqttEvent;

// region variables
const WIFI_SSID: &str = "Wokwi-GUEST";
const WIFI_PASSWORD: &str = "";
//broker.mqttdashboard.com
const MQTT_BROKER_URL: &str = "mqtt://broker.mqttdashboard.com:1883";
// const MQTT_BROKER_URL: &str = "mqtts://6804c12a8e254e5c9f0d45b0ea9c0b2a.s1.eu.hivemq.cloud:8883";
// const MQTT_PORT: u16 = 8883;
const MQTT_TOPIC: &str = "testtopic/1";
// const MQTT_CLIENT_ID: &str = "esp32";
// const MQTT_USERNAME: &str = "hivemq.webclient.1726606709307";
// const MQTT_PASSWORD: &str = "9wD6c7YmrP<tIi!5VL#>";

// endregion

#[derive(Deserialize)]
struct FormData {
    message: String<128>,
}

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once.
    // Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly.
    // See https://github.com/esp-rs/esp-idf-template/issues/71
    // for more information.
    // Try to uncomment the line below and see if the build fails.
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::set_max_level(log::LevelFilter::Debug);

    let mut server = create_server()?;

    info!("Server started");

    // Create MQTT Connection
    info!("Creating MQTT Connection");

    // Create MQTT Client Configuration
    connect_mqtt()?;
    // let mqtt_config = MqttClientConfiguration::default();

    // // let mqtt_config = MqttClientConfiguration {
    // //     client_id: Some(MQTT_CLIENT_ID.into()),
    // //     username: Some(MQTT_USERNAME.into()),
    // //     password: Some(MQTT_PASSWORD.into()),
    // //     keep_alive_interval: Some(Duration::from_secs(60)),
    // //     ..Default::default()
    // // };

    // info!("MQTT Configuration Created to broker: {}", MQTT_BROKER_URL);

    // // Create MQTT Client
    // // let (mut client, mut connection) = EspMqttClient::new(MQTT_BROKER_URL, &mqtt_config)?;

    // let (mut client, mut connection) = EspMqttClient::new(
    //     "mqtt://broker.mqttdashboard.com",
    //     &mqtt_config)?;

    // info!("MQTT Connection Created");

    // // Connect to MQTT Broker
    // info!("Connected to MQTT Broker");

    // // Subscribe to MQTT Topic
    // client.subscribe(MQTT_TOPIC.into(), QoS::AtLeastOnce)?;
    // info!("Subscribed to MQTT Topic");

    // // Publish to MQTT Topic
    // client.publish(
    //     MQTT_TOPIC.into(),
    //     QoS::AtLeastOnce,
    //     false,
    //     "Hello World".as_bytes(),
    // )?;

    // info!("Published to MQTT Topic");

    // Wait for messages
    // loop {
    //     let message = connection.receive()?;
    //     info!("Received message: {:?}", message);
    // }

    server.fn_handler("/", Method::Get, |req| {
        req.into_ok_response()?
            .write_all(create_html_response().as_bytes())
            .map(|_| ())
    })?;

    // add route for publishing message to mqtt broker when the data is sent from the form
    server.fn_handler("/", Method::Post, move |mut req| -> anyhow::Result<()> {
        let mut body = Vec::new();
        req.read(&mut body)?;

        let mut body_string: String<1024> = String::new();
        body_string.push_str(std::str::from_utf8(&body).unwrap()).unwrap();

        req.into_ok_response()?;
        let form_data: FormData = serde_urlencoded::from_bytes(&body).unwrap();
        let message = form_data.message;
        
        // let (mut client, mut connection) = EspMqttClient::new(MQTT_BROKER_URL, &mqtt_config)?;

        // client.publish(
        //     MQTT_TOPIC.into(),
        //     QoS::AtLeastOnce,
        //     false,
        //     message.as_bytes(),
        // )?;

        Ok(())
    })?;
    info!("Server routes added");

    // core::mem::forget(wifi);
    core::mem::forget(server);
    // core::mem::forget(client);

    // Main task no longer needed, free up some memory
    Ok(())
}

// load the html form to be displayed on the web page when the server is accessed via a browser or client application
fn create_html_response() -> String<1024> {
    // the html page is in a external file
    let html = include_str!("index.html");
    let mut html_string: String<1024> = String::new();
    html_string.push_str(html).unwrap();
    return html_string;
}

fn create_server() -> anyhow::Result<EspHttpServer<'static>> {
    let peripherals = Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    // configurare wifi in modalità client
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;

    connect_wifi(&mut wifi)?;

    info!(
        "Connected Wi-Fi with WIFI_SSID `{}` and WIFI_PASS `{}`",
        WIFI_SSID, WIFI_PASSWORD
    );

    let server_configuration = esp_idf_svc::http::server::Configuration {
        ..Default::default()
    };

    // Keep wifi running beyond when this function returns (forever)
    // Do not call this if you ever want to stop or access it later.
    // Otherwise it should be returned from this function and kept somewhere
    // so it does not go out of scope.
    // https://doc.rust-lang.org/stable/core/mem/fn.forget.html
    core::mem::forget(wifi);

    Ok(EspHttpServer::new(&server_configuration)?)
}

// Connect to Wi-Fi network with the provided SSID and password. This function is blocking.
// It will return once the Wi-Fi connection is established. If the connection fails, an error will be returned.
// The Wi-Fi connection is established using the provided `wifi` instance.
// The Wi-Fi configuration is set to client mode with the provided SSID and password.
// The Wi-Fi connection is started, and the connection is established.
// The network interface is then brought up.
// If the connection is successful, the function will return `Ok(())`.
// If the connection fails, an error will be returned.
fn connect_wifi(wifi: &mut BlockingWifi<EspWifi<'static>>) -> anyhow::Result<()> {
    let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
        ssid: WIFI_SSID.try_into().unwrap(),
        // bssid: None,
        // auth_method: AuthMethod::WPA2Personal,
        auth_method: AuthMethod::None,
        password: WIFI_PASSWORD.try_into().unwrap(),
        channel: None,
        ..Default::default()
    });

    wifi.set_configuration(&wifi_configuration)?;

    wifi.start()?;
    info!("Wifi started successfully");

    // Connect in dhcp mode with default timeout of 10 seconds
    wifi.connect()?;
    info!("Wifi connected");

    // Wait until the network interface is up with default timeout of 10 seconds
    wifi.wait_netif_up()?;
    info!("Wifi netif up");


    // Print Out Wifi Connection Configuration
    while !wifi.is_connected().unwrap() {
        // Get and print connection configuration
        let config = wifi.get_configuration().unwrap();
        info!("Waiting for station {:?}", config);
    }

    info!("Wifi Connected");

    Ok(())
}

fn connect_mqtt() -> Result<(), anyhow::Error> {
    let conf = MqttClientConfiguration {
        client_id: Some("test_client_id"),
        ..Default::default()
    };
    let (mut client, mut connection) = EspMqttClient::new(MQTT_BROKER_URL, &conf)?;
    info!("Connected to MQTT Broker");
    
    client.publish(MQTT_TOPIC, QoS::AtLeastOnce, false, b"hello world")?;

    info!("Message published");
    // Ensure the connection loop is running
    while let Ok(msg) = connection.next() {
        info!("New message received");
        // match msg {
        //     EspMqttEvent::Received(received) => {
        //         println!("Received message on topic: {}", received.topic());
        //         println!("Message payload: {:?}", received.data());
        //     }
        //     _ => {
        //         println!("Other MQTT event");
        //     }
        // }
    }
    Ok(())
}