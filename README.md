# NymView

## Overview
**NymView** is a fork of a proof-of-concept project that demonstrates hosting Markdown pages over the [Nym Mixnet](https://nymtech.net/).  
It allows you to serve static Markdown content in a **privacy-preserving** way.

---

## Credits
This project is based on the original work by [Ch1ffr3punk/NymView](https://github.com/Ch1ffr3punk/NymView).

---

As mentioned earlier, NymView is a fork of a proof-of-concept project that demonstrates hosting Markdown pages over the Nym Mixnet. It enables you to serve static Markdown content in a privacy-preserving way, many features are planned for the future, including multiple tab support, gateway selection for both server and browser, and various general improvements. In the meantime, you can try it out by creating and host your own personal page over nym mixnet.



## Getting Started

### Build from Source
To build NymView compoments from source, follow these steps.  
```bash
# Clone the repository
git clone https://github.com/valansai/NymView.git
cd NymView

# Build the components
cargo build --release
```
This will create two binaries:

Server binary – hosts Markdown pages over the Nym Mixnet

Browser binary – used to access and view pages via the Nym Mixnet


## Usage

### Launch NymView browser
``` bash
target/release/nym-view-client
```
- Once launched, enter a Nym address in the search bar. And click go.
   
![Browser](https://iili.io/fFpUX8Q.png)

By design, NymView serves Markdown content, and the server returns a default page if no content has been added.

![BrowserDefaultPage](https://iili.io/fFphEdX.png)

### Start serving a webpage 
``` bash
target/release/nym-view-server
```

This command will start the server, and you should see output similar to the following:

```bash
Persistence directory: "/home/rootx/.config/NymView/mixnet_server"
NymView Server started: nym://EvchCCf8k1k5nM2Xysu3LEpto4xVQpEKz9sRQkC8xB6K.GcNNjHz1YVK1aB3bTY9dHJkepAdsSvBgWtNBanpvmQCE@DK46aDSsaYJsqmRPihWDokcJpcgvAZQecEZ75WdNXsn4
```

The persistence directory stores your nym client cryptographic keys, that means even if you shut down or restart the server, anyone who has your NymView Server address will still be able to reach it under the same address.


To let others access your server, share your server address.
Anyone using the NymView browser can connect to your server using this address.

***Note***: By default, visitors will see a built-in default page served by your server.

### How to add your own page 

Example Markdown template for a personal “About Me” page

```
# 👋 Hello, I'm Your Name

---

# 📝 About Me

Write a short introduction about who you are and what you do.

Describe your interests, background, or what you enjoy learning.

---

## 👤 Who I Am
- List a personal trait or quality  
- List another personal trait or quality  

---

---

## ⚡ Fun Fact
Write a fun fact, unique detail, or statement about yourself.

---

### 🖋️ Interests
- Interest 1  
- Interest 2  
- Interest 3  

---

```

How to Use This Template:
- Copy this template and paste it into a text editor. Or create your own markdown template. 
- Fill in each section with your own information to create your personal page. You can also remove sections. 
- Save the file as index.md inside the pages folder of your NymView directory.
- Start the server ```target/release/nym-view-server```
- Use the NymView browser yourself or share your server address—anyone who visits it will be able to see your personal page.


An example of how a personal page looks when visited, assuming the above template is used.

![BrowserPersonalPage](https://iili.io/fKHbNHB.png)

 



