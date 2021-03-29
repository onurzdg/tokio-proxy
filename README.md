Description
-----------
- A Proxy server that Relies on Tokio to scale to heavy load via green threads
- Establishes tunnels via HTTP Connect handshake   
- Creates short-lived tunnels to provide fairness to all clients
- Uses an optional whitelist to only route requests to a few sites mentioned in the config.

Things to Improve
-----------------
- Read server configuration parameters from a YAML file and replace hardcoded parameters in the code
- Read parameters from command line to select port and max allowed connection limit
- Write end-to-end tests  
- Use a DNS resolver and cache IPs of accessed sites 
- Replace calls to "tokio::copy(src, dst)" with a custom loop to be able to accurately report the 
  amount of bytes transferred while working with a timeout.
- Stress testing and tweaking to find ideal server parameters around timeouts


Running
--------
 1. cargo build && cargo run 
 2. Set proxy settings in Firefox to 127.0.0.1:12345
 3. Browse gfycat.com and giphy.com



