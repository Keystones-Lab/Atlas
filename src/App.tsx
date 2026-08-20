import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [systemInfo, setSystemInfo] = useState(null)

  async function getSystemInfo() {
    try{
    setSystemInfo(await invoke("get_system_info"))
    }
    catch(error){
      console.error("Failed to connect to the backend", error)
    }
  }
  console.log("This is to check what i am receving", systemInfo)

  return (
    <main className="container">
      <button onClick={getSystemInfo}>Get Diagnostics</button>
      {systemInfo === null ? 
      <p>There is no data</p> : 
      <>
      <p>CPU: {systemInfo.cpu_percent}%</p>
      <p>RAM: {systemInfo.used_memory} / {systemInfo.total_memory}</p>
      <p>DISK: {systemInfo.used_disk} / {systemInfo.total_disk}</p>
      </>
    }
    </main>
  );
}

export default App;
