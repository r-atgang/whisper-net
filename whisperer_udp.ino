// SPDX-License-Identifier: Apache-2.0
//
// Copyright 2025 ratgang
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//

#include <WiFi.h>
#include <WiFiUdp.h>
#include <driver/i2s.h>

// === Network Configuration ===
const char* ssid_c2 = "NETGEAR69";              // Replace with your Wi-Fi SSID
const char* password_c2 = "ChangeThis000$";          // Replace with your Wi-Fi password
const char* address_UDP = "10.42.0.1";  // Replace with laptop/server IP
const int port_UDP = 6969;                  // UDP port for audio data

// === I2S Mic Configuration ===
#define ws_pin     18  // LRCLK / WS
#define sck_pin    19  // BCLK / SCK
#define sd_pin     22  // DOUT

const int SAMPLE_RATE = 16000;
const int buffer_samples = 1024;            // Number of samples per buffer

WiFiUDP udp;

void setup() {
  Serial.begin(115200);

  // === Connect to Wi-Fi ===
  WiFi.begin(ssid_c2, password_c2);
  Serial.print("Connecting to Wi-Fi");
  while (WiFi.status() != WL_CONNECTED) {
    delay(500);
    Serial.print(".");
  }
  Serial.println("\nWi-Fi connected.");
  Serial.print("ESP32 IP: ");
  Serial.println(WiFi.localIP());

  // Start UDP socket
  udp.begin(12345);  // Local port (arbitrary)

  // === Configure I2S ===
  const i2s_config_t i2s_config = {
    (i2s_mode_t)(I2S_MODE_MASTER | I2S_MODE_RX),  // Explicit cast required
    SAMPLE_RATE,
    I2S_BITS_PER_SAMPLE_32BIT,
    I2S_CHANNEL_FMT_ONLY_LEFT,
    I2S_COMM_FORMAT_STAND_I2S,
    ESP_INTR_FLAG_LEVEL1,
    4,
    buffer_samples,
    false,
    false,
    -1
  };

  const i2s_pin_config_t pin_config = {
    .bck_io_num = sck_pin,          // BCLK
    .ws_io_num = ws_pin,           // LRCLK
    .data_out_num = I2S_PIN_NO_CHANGE,// Not used (data out)
    .data_in_num = sd_pin,            // DOUT (data in from mic)
  };

  if (i2s_driver_install(I2S_NUM_0, &i2s_config, 0, NULL) != ESP_OK) {
    Serial.println("Error setting up I2S driver.");
    while (true);
  }

  if (i2s_set_pin(I2S_NUM_0, &pin_config) != ESP_OK) {
    Serial.println("Error setting I2S pins.");
    while (true);
  }

  Serial.println("I2S initialized successfully.");
}

void loop() {
  static int32_t samples[buffer_samples];       // 32-bit raw samples
  static int16_t samples_16bit[buffer_samples]; // 16-bit downsampled

  size_t bytes_read = 0;

  // Read data from I2S
  Serial.println("Reading I2S samples...");
  i2s_read(I2S_NUM_0, samples, sizeof(samples), &bytes_read, portMAX_DELAY);

  if (bytes_read > 0) {
    Serial.print("Read ");
    Serial.print(bytes_read);
    Serial.println(" bytes from I2S.");
    int num_samples = bytes_read / sizeof(int32_t);

    // Convert 32-bit signed samples to 16-bit
    for (int i = 0; i < num_samples; i++) {
      samples_16bit[i] = samples[i] >> 16;
    }

    // Send over UDP
    Serial.println("Sending data via UDP...");
    udp.beginPacket(address_UDP, port_UDP);
    udp.write((uint8_t*)samples_16bit, num_samples * sizeof(int16_t));
    int result = udp.endPacket();

    if (result != 1) {
      Serial.println("Error sending UDP packet.");
    } else {
      Serial.println("UDP packet sent successfully.");
    }
  }
}
