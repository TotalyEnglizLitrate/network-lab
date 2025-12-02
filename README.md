# Minimal Network Lab with QEMU Overlays

## Objective
Build a minimal prototype to create/manage virtual nodes in a network lab, using **QEMU disk overlays** and **Guacamole**.

## Features
- **Create & Manage Virtual Nodes**
- **Node Lifecycle Management**
- **Guacamole Integration**

## Setup Instructions
1. **Clone the Repository**
2. **Backend Setup**
3. **Frontend Setup**
4. **Docker Setup for Guacamole**
5. **Run the Backend**
6. **Run the Frontend**
7. **Access the Application**

## API Endpoints
- **POST** `/nodes`
- **POST** `/nodes/:id/run`
- **POST** `/nodes/:id/stop`
- **POST** `/nodes/:id/wipe`
- **GET** `/nodes`

## Evaluation Criteria
- Node lifecycle works (Run / Stop / Wipe)
- Uses QEMU overlays efficiently
- Guacamole console works on click
- Bonus: Multiple nodes running concurrently
