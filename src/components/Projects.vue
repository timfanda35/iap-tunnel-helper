<script setup>
import { ref } from "vue";
import { Command } from '@tauri-apps/api/shell';
import { confirm } from '@tauri-apps/api/dialog';

/*
In Mac OS, we can check the port listen status

    watch  -c "lsof -n -i -P | grep LISTEN"

The parameters of lsof:
  -n no host names
  -i select IPv[46] files
  -P no port names
*/

const email = ref("");

const projects = ref([]);
const selectedProjectId = ref("");

const gceInstances = ref([]);
const selectedGceInstance = ref("");

const localPort = ref("");
const port = ref("");

const iapTunnelCommands = ref({});

async function loadAccount() {
  const command = new Command('get-google-account');
  command.on('close', data => {
    console.log(`command finished with code ${data.code} and signal ${data.signal}`)
  });
  command.on('error', error => console.error(`command error: "${error}"`));
  command.stdout.on('data', line => email.value = line);

  command.spawn();
}

async function loadProjects() {
  let outputLines = [];
  const command = new Command('get-google-projects');
  command.on('close', data => {
    let output = outputLines.join('');
    let parsedOutput = JSON.parse(output);

    // filter system project
    parsedOutput = parsedOutput.filter((project) => !project.projectId.startsWith('sys'));

    projects.value = parsedOutput.sort((a, b) => a.name > b.name ? 1 : -1);

    if (projects.value.length > 0) {
      selectedProjectId.value = projects.value[0].projectId;
      loadGCEInstances();
    } else {
      selectedProjectId.value = '';
    }
  });
  command.on('error', error => console.error(`command error: "${error}"`));
  command.stdout.on('data', line => outputLines.push(line));

  command.spawn();
}

async function loadGCEInstances() {
  let outputLines = [];
  const args = ["compute", "instances", "list", "--format=json", "--project", selectedProjectId.value];
  const command = new Command('get-gce-instances', args);
  command.on('close', data => {
    let output = outputLines.join('');
    let parsedOutput = JSON.parse(output);

    gceInstances.value = parsedOutput.sort((a, b) => a.name > b.name ? 1 : -1);

    if (gceInstances.value.length > 0) {
      selectedGceInstance.value = gceInstances.value[0];
    } else {
      selectedGceInstance.value = null;
    }
  });
  command.on('error', error => console.error(`command error: "${error}"`));
  command.stdout.on('data', line => outputLines.push(line));

  command.spawn();
}

async function startIAPTunnel() {
  if (selectedGceInstance.value === null) {
    console.warn("no gce instance selected");
    alert("no gce instance selected");
    return;
  }

  const args = [
    "compute",
    "start-iap-tunnel",
    "--project", selectedProjectId.value,
    "--zone", shortZone(selectedGceInstance.value.zone),
    "--local-host-port", `localhost:${localPort.value}`,
    selectedGceInstance.value.name,
    port.value
  ];

  // Force flush python stdout and stderr
  // https://www.delftstack.com/howto/python/python-print-flush/#flush-print-output-in-python-using-the--u-flag
  // 
  // Google Cloud SDK does not flush stdout
  // 
  // lib/googlecloudsdk/command_lib/compute/iap_tunnel.py:707
  // 
  //      log.out.Print('Listening on port [%d].' % self._local_port)
  // 
  // lib/googlecloudsdk/core/log.py:573
  //      
  //      self.stdout_writer = _ConsoleWriter(self.file_only_logger,
  //                                          self._user_output_filter,
  //                                          self.stdout_stream_wrapper)
  // 
  const spawnOptions = {
    env: {
      "CLOUDSDK_PYTHON_ARGS": "-u"
    }
  };

  const command = new Command('start-iap-tunnel', args, spawnOptions);
  let child;
  command.on('close', data => {
    console.log(`Tunnel down instance: ${selectedGceInstance.value.name}:${port.value}`);
    delete iapTunnelCommands.value[child.pid];
  });
  command.on('error', error => console.error(`command error: "${error}"`));
  command.stdout.on('data', line => {
    console.log(line);

    if (line.startsWith('Listening on port')) {
      iapTunnelCommands.value[child.pid].isConnected = true;    
    }
  });
  command.stderr.on('data', line => console.error(line));

  child = await command.spawn();

  iapTunnelCommands.value[child.pid] = {
    projectId: selectedProjectId.value,
    zone: shortZone(selectedGceInstance.value.zone),
    instanceName: selectedGceInstance.value.name,
    localPort: localPort.value,
    port: port.value,
    childProcess: child,
    isConnected: false
  };
}

async function stopIAPTunnel(iapTunnel) {
  const confirmed = await confirm(`Do your want to stop iap tunnel to ${iapTunnel.instanceName}:${iapTunnel.port}?`);

  if (confirmed) {
    iapTunnel.childProcess.kill();
  }
}

function tunnelStatus(iapTunnel) {
  return iapTunnel.isConnected ? 'online' : 'offline'; 
}

async function reload() {
  console.log('reload');
  checkGcloud();
  loadAccount();
  loadProjects();
} 

async function updateSelectedProjectId(event) {
  selectedProjectId.value = event.target.value;
  loadGCEInstances();
}

async function updateSelectedGceInstance(event) {
  selectedGceInstance.value = gceInstances.value.find((inst) => inst.id === event.target.value);
}

function shortZone(zone) {
  return zone.split('/').pop();
}

async function checkGcloud() {
  const command = new Command('check-gcloud');
  command.on('error', error => console.error(`command error: "${error}"`));
  command.stdout.on('data', line => console.error(line));
  command.stderr.on('data', line => console.error(line));

  await command.spawn();
}

reload();

</script>

<template>
  <div class="config-wrap">
    <div class="row">
      <div class="item-label">
        Account
      </div>
      <div class="item-wrap">
        <div>
          {{ email }}
        </div>
        <div>
          <button type="button" @click="reload()">ReLoad</button>
        </div>
      </div>
    </div>
    <div class="row">
      <div class="item-label">Projects: {{ projects.length }}</div>
      <div class="item-select-wrap">
        <select class="select-fine" @change="updateSelectedProjectId">
          <template v-for="project in projects" :key="project.projectId">
            <option :value="project.projectId">{{ project.name }} ({{ project.projectId }})</option>
          </template>
        </select>
      </div>
    </div>
    
    <div class="row">
      <div class="item-label">GCE Instances: {{ gceInstances.length }}</div>
      <div class="item-select-wrap">
        <select class="select-fine" @change="updateSelectedGceInstance">
          <template v-for="gceInstance in gceInstances" :key="gceInstance.id">
            <option :value="gceInstance.id">{{ gceInstance.name }} ({{ shortZone(gceInstance.zone) }})</option>
          </template>
        </select>
      </div>
    </div>

    <div class="row">
      <div class="item-label">
        Local Port
      </div>
      <div class="input-port-wrap">
        <input type="text" v-model="localPort" class="input-port" />
      </div>
    </div>
    <div class="row">
      <div class="item-label">
        Instance Port
      </div>
      <div class="input-port-wrap">
        <input type="text" v-model="port" class="input-port" />
      </div>
    </div>

    <div class="row">
      <div class="start-button-wrap">
        <button type="button" @click="startIAPTunnel()">Start IAP Tunnel</button>
      </div>
    </div>
  </div>

  <div>
    <h2>IAP Tunnels</h2>
    <hr />
    <template v-for="(iapTunnel, pid) in iapTunnelCommands" :key="pid">
      <div>
        <button @click="stopIAPTunnel(iapTunnel)">
          <span :class="{ 'tunnel-status-connected': iapTunnel.isConnected }">[{{ tunnelStatus(iapTunnel) }}]</span>
          localhost:{{ iapTunnel.localPort }} -> {{iapTunnel.projectId}}/{{iapTunnel.instanceName}}:{{iapTunnel.port}}
        </button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.config-wrap {
  border-width: 1px;
  border-style: solid;
  border-radius: 3rem;

  padding-top: 1rem;
  padding-bottom:  1rem;
}

.item-label {
  text-align: left;
  width: 10rem;

  display: flex;
  align-items: center;
}

.item-wrap {
  width: 35rem;

  display: flex;
  justify-content: space-between;
  align-items: center;
}

.item-select-wrap {
  width: 35rem;
}

.select-fine {
  font-size: 5rem;
  width: 100%;
}

.input-port-wrap {
  text-align: left;
  width: 35rem;
}

.input-port {
  width: 5rem;
}

.start-button-wrap {
  width: 15rem;
}

.tunnel-status-connected {
  color: green;
}

</style>