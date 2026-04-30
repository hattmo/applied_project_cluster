import { useState, useEffect } from 'react'
import './App.css'

// Types
interface MatrixUser {
  user_id: string
  display_name: string | null
}

interface VmConfig {
  id: string
  name: string
  user_id: string
  enabled: boolean
  created_at: string
  updated_at: string
}

interface Task {
  description: string
  keystrokes?: string
  delay_ms?: number
}

interface TaskQueue {
  id: string
  vm_id: string
  name: string
  tasks: Task[]
  enabled: boolean
  created_at: string
  updated_at: string
}

// API base URL
const API_BASE = '/api/v1'

function App() {
  const [activeTab, setActiveTab] = useState<'vms' | 'queues'>('vms')
  
  // Matrix agents state
  const [agents, setAgents] = useState<MatrixUser[]>([])
  const [agentsLoading, setAgentsLoading] = useState(true)
  const [agentsError, setAgentsError] = useState<string | null>(null)
  
  // VM Configs state
  const [vmConfigs, setVmConfigs] = useState<VmConfig[]>([])
  const [vmLoading, setVmLoading] = useState(true)
  const [vmError, setVmError] = useState<string | null>(null)
  const [showVmForm, setShowVmForm] = useState(false)
  const [newVmName, setNewVmName] = useState('')
  const [newVmUserId, setNewVmUserId] = useState('')
  const [editingVm, setEditingVm] = useState<VmConfig | null>(null)
  
  // Available VMs from vmware_gateway
  const [availableVms, setAvailableVms] = useState<string[]>([])
  const [availableVmsLoading, setAvailableVmsLoading] = useState(true)

  // Task Queues state
  const [taskQueues, setTaskQueues] = useState<TaskQueue[]>([])
  const [queueLoading, setQueueLoading] = useState(true)
  const [queueError, setQueueError] = useState<string | null>(null)
  const [showQueueForm, setShowQueueForm] = useState(false)
  const [newQueueName, setNewQueueName] = useState('')
  const [newQueueVmId, setNewQueueVmId] = useState('')
  const [editingQueue, setEditingQueue] = useState<TaskQueue | null>(null)
  
  // New task state
  const [newTaskDescription, setNewTaskDescription] = useState('')
  const [newTaskKeystrokes, setNewTaskKeystrokes] = useState('')
  const [newTaskDelay, setNewTaskDelay] = useState('')

  // Load agents and available VMs
  useEffect(() => {
    fetchAgents()
    fetchAvailableVms()
  }, [])

  // Load VM Configs
  useEffect(() => {
    fetchVmConfigs()
  }, [])

  // Load Task Queues when switching to queues tab
  useEffect(() => {
    if (activeTab === 'queues') {
      fetchTaskQueues()
    }
  }, [activeTab])

  async function fetchAgents() {
    try {
      const res = await fetch(`${API_BASE}/agents`)
      if (!res.ok) throw new Error('Failed to fetch agents')
      const data = await res.json()
      setAgents(data)
      setAgentsLoading(false)
    } catch (err) {
      setAgentsError(err instanceof Error ? err.message : 'Unknown error')
      setAgentsLoading(false)
    }
  }

  async function fetchAvailableVms() {
    try {
      const res = await fetch(`${API_BASE}/vms`)
      if (!res.ok) throw new Error('Failed to fetch available VMs')
      const data = await res.json()
      setAvailableVms(data)
      setAvailableVmsLoading(false)
    } catch (err) {
      console.error('Failed to fetch available VMs:', err)
      setAvailableVmsLoading(false)
    }
  }

  async function fetchVmConfigs() {
    try {
      const res = await fetch(`${API_BASE}/vm-configs`)
      if (!res.ok) throw new Error('Failed to fetch VM configs')
      const data = await res.json()
      setVmConfigs(data)
      setVmLoading(false)
    } catch (err) {
      setVmError(err instanceof Error ? err.message : 'Unknown error')
      setVmLoading(false)
    }
  }

  async function fetchTaskQueues() {
    try {
      const res = await fetch(`${API_BASE}/task-queues`)
      if (!res.ok) throw new Error('Failed to fetch task queues')
      const data = await res.json()
      setTaskQueues(data)
      setQueueLoading(false)
    } catch (err) {
      setQueueError(err instanceof Error ? err.message : 'Unknown error')
      setQueueLoading(false)
    }
  }

  async function createVmConfig() {
    try {
      const res = await fetch(`${API_BASE}/vm-configs`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: newVmName, user_id: newVmUserId, enabled: true }),
      })
      if (!res.ok) throw new Error('Failed to create VM config')
      await fetchVmConfigs()
      setNewVmName('')
      setNewVmUserId('')
      setShowVmForm(false)
    } catch (err) {
      setVmError(err instanceof Error ? err.message : 'Unknown error')
    }
  }

  async function updateVmConfig(id: string, updates: Partial<VmConfig>) {
    try {
      const res = await fetch(`${API_BASE}/vm-configs/${id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(updates),
      })
      if (!res.ok) throw new Error('Failed to update VM config')
      await fetchVmConfigs()
      setEditingVm(null)
    } catch (err) {
      setVmError(err instanceof Error ? err.message : 'Unknown error')
    }
  }

  async function deleteVmConfig(id: string) {
    if (!confirm('Delete this VM config?')) return
    try {
      const res = await fetch(`${API_BASE}/vm-configs/${id}`, { method: 'DELETE' })
      if (!res.ok) throw new Error('Failed to delete VM config')
      await fetchVmConfigs()
    } catch (err) {
      setVmError(err instanceof Error ? err.message : 'Unknown error')
    }
  }

  async function createTaskQueue() {
    try {
      const res = await fetch(`${API_BASE}/task-queues`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ 
          vm_id: newQueueVmId, 
          name: newQueueName, 
          tasks: [],
          enabled: true
        }),
      })
      if (!res.ok) throw new Error('Failed to create task queue')
      await fetchTaskQueues()
      setNewQueueName('')
      setNewQueueVmId('')
      setShowQueueForm(false)
    } catch (err) {
      setQueueError(err instanceof Error ? err.message : 'Unknown error')
    }
  }

  async function updateTaskQueue(id: string, updates: Partial<TaskQueue>) {
    try {
      const res = await fetch(`${API_BASE}/task-queues/${id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(updates),
      })
      if (!res.ok) throw new Error('Failed to update task queue')
      await fetchTaskQueues()
      setEditingQueue(null)
    } catch (err) {
      setQueueError(err instanceof Error ? err.message : 'Unknown error')
    }
  }

  async function deleteTaskQueue(id: string) {
    if (!confirm('Delete this task queue?')) return
    try {
      const res = await fetch(`${API_BASE}/task-queues/${id}`, { method: 'DELETE' })
      if (!res.ok) throw new Error('Failed to delete task queue')
      await fetchTaskQueues()
    } catch (err) {
      setQueueError(err instanceof Error ? err.message : 'Unknown error')
    }
  }

  async function addTaskToQueue(queueId: string) {
    if (!newTaskDescription.trim()) return
    try {
      const queue = taskQueues.find(q => q.id === queueId)
      if (!queue) throw new Error('Queue not found')
      
      const newTask = {
        description: newTaskDescription,
        keystrokes: newTaskKeystrokes || undefined,
        delay_ms: newTaskDelay ? parseInt(newTaskDelay) : undefined,
      }
      
      const res = await fetch(`${API_BASE}/task-queues/${queueId}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          ...queue,
          tasks: [...queue.tasks, newTask],
        }),
      })
      if (!res.ok) throw new Error('Failed to add task')
      await fetchTaskQueues()
      setNewTaskDescription('')
      setNewTaskKeystrokes('')
      setNewTaskDelay('')
    } catch (err) {
      setQueueError(err instanceof Error ? err.message : 'Unknown error')
    }
  }

  async function deleteTaskFromQueue(queueId: string, taskIndex: number) {
    try {
      const queue = taskQueues.find(q => q.id === queueId)
      if (!queue) throw new Error('Queue not found')
      
      const updatedTasks = queue.tasks.filter((_, idx) => idx !== taskIndex)
      
      const res = await fetch(`${API_BASE}/task-queues/${queueId}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          ...queue,
          tasks: updatedTasks,
        }),
      })
      if (!res.ok) throw new Error('Failed to delete task')
      await fetchTaskQueues()
    } catch (err) {
      setQueueError(err instanceof Error ? err.message : 'Unknown error')
    }
  }

  function getVmName(vmId: string) {
    const vm = vmConfigs.find(v => v.id === vmId)
    return vm ? vm.name : vmId
  }

  function getAgentDisplayName(userId: string) {
    const agent = agents.find(a => a.user_id === userId)
    if (!agent) return userId
    return agent.display_name || agent.user_id
  }

  return (
    <div className="container">
      <h1>🖥️ NPC VM Operator</h1>
      
      <div className="tabs">
        <button 
          className={`tab ${activeTab === 'vms' ? 'active' : ''}`}
          onClick={() => setActiveTab('vms')}
        >
          Agent Assignments
        </button>
        <button 
          className={`tab ${activeTab === 'queues' ? 'active' : ''}`}
          onClick={() => setActiveTab('queues')}
        >
          Task Queues
        </button>
      </div>

      {/* Agent Assignments Tab */}
      {activeTab === 'vms' && (
        <div className="card">
          <div className="card-header">
            <h2>Agent Assignments</h2>
            <button className="btn btn-primary" onClick={() => setShowVmForm(!showVmForm)}>
              {showVmForm ? 'Cancel' : '+ Add VM'}
            </button>
          </div>

          {showVmForm && (
            <div className="form">
              <select
                value={newVmName}
                onChange={(e) => setNewVmName(e.target.value)}
                className="input"
              >
                <option value="">Select VM</option>
                {availableVmsLoading ? (
                  <option disabled>Loading VMs...</option>
                ) : availableVms.length === 0 ? (
                  <option disabled>No VMs available</option>
                ) : (
                  availableVms.map((vm) => (
                    <option key={vm} value={vm}>{vm}</option>
                  ))
                )}
              </select>
              <select
                value={newVmUserId}
                onChange={(e) => setNewVmUserId(e.target.value)}
                className="input"
              >
                <option value="">Select User</option>
                {agentsLoading ? (
                  <option disabled>Loading users...</option>
                ) : agentsError ? (
                  <option disabled>Error loading users</option>
                ) : agents.length === 0 ? (
                  <option disabled>No users available</option>
                ) : (
                  agents.map((agent) => (
                    <option key={agent.user_id} value={agent.user_id}>
                      {agent.display_name || agent.user_id}
                    </option>
                  ))
                )}
              </select>
              <button className="btn btn-primary" onClick={createVmConfig}>Create</button>
            </div>
          )}

          {vmLoading ? (
            <p>Loading...</p>
          ) : vmError ? (
            <p className="error">Error: {vmError}</p>
          ) : vmConfigs.length === 0 ? (
            <p className="empty">No VM configs yet. Add one to get started!</p>
          ) : (
            <div className="list">
              {vmConfigs.map((vm) => (
                <div key={vm.id} className="list-item">
                  {editingVm?.id === vm.id ? (
                    <div className="edit-form">
                      <select
                        defaultValue={vm.name}
                        onBlur={(e) => updateVmConfig(vm.id, { name: e.target.value })}
                        className="input"
                      >
                        {availableVms.map((availVm) => (
                          <option key={availVm} value={availVm}>{availVm}</option>
                        ))}
                      </select>
                      <select
                        defaultValue={vm.user_id}
                        onBlur={(e) => updateVmConfig(vm.id, { user_id: e.target.value })}
                        className="input"
                      >
                        {agents.map((agent) => (
                          <option key={agent.user_id} value={agent.user_id}>
                            {agent.display_name || agent.user_id}
                          </option>
                        ))}
                      </select>
                    </div>
                  ) : (
                    <div className="item-content">
                      <div>
                        <strong>{vm.name}</strong>
                        <span className="badge">User: {getAgentDisplayName(vm.user_id)}</span>
                      </div>
                      <span className={`status ${vm.enabled ? 'active' : 'inactive'}`}>
                        {vm.enabled ? '● Active' : '○ Inactive'}
                      </span>
                    </div>
                  )}
                  <div className="item-actions">
                    {editingVm?.id === vm.id ? (
                      <button className="btn btn-small" onClick={() => setEditingVm(null)}>Done</button>
                    ) : (
                      <button className="btn btn-small" onClick={() => setEditingVm(vm)}>Edit</button>
                    )}
                    <button 
                      className="btn btn-small"
                      onClick={() => updateVmConfig(vm.id, { enabled: !vm.enabled })}
                    >
                      {vm.enabled ? 'Disable' : 'Enable'}
                    </button>
                    <button 
                      className="btn btn-small btn-danger"
                      onClick={() => deleteVmConfig(vm.id)}
                    >
                      Delete
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Task Queues Tab */}
      {activeTab === 'queues' && (
        <div className="card">
          <div className="card-header">
            <h2>Task Queues</h2>
            <button className="btn btn-primary" onClick={() => setShowQueueForm(!showQueueForm)}>
              {showQueueForm ? 'Cancel' : '+ Add Queue'}
            </button>
          </div>

          {showQueueForm && (
            <div className="form">
              <select
                value={newQueueVmId}
                onChange={(e) => setNewQueueVmId(e.target.value)}
                className="input"
              >
                <option value="">Select VM</option>
                {vmConfigs.map((vm) => (
                  <option key={vm.id} value={vm.id}>{vm.name}</option>
                ))}
              </select>
              <input
                type="text"
                placeholder="Queue Name"
                value={newQueueName}
                onChange={(e) => setNewQueueName(e.target.value)}
                className="input"
              />

              <button className="btn btn-primary" onClick={createTaskQueue}>Create</button>
            </div>
          )}

          {queueLoading ? (
            <p>Loading...</p>
          ) : queueError ? (
            <p className="error">Error: {queueError}</p>
          ) : taskQueues.length === 0 ? (
            <p className="empty">No task queues yet. Add one to get started!</p>
          ) : (
            <div className="list">
              {taskQueues.map((queue) => (
                <div key={queue.id} className="list-item list-item-expanded">
                  <div className="item-header">
                    {editingQueue?.id === queue.id ? (
                      <input
                        type="text"
                        defaultValue={queue.name}
                        onBlur={(e) => updateTaskQueue(queue.id, { name: e.target.value })}
                        className="input"
                      />
                    ) : (
                      <strong>{queue.name}</strong>
                    )}
                    <div className="item-meta">
                      <span className="badge">VM: {getVmName(queue.vm_id)}</span>
                      <span className={`status ${queue.enabled ? 'active' : 'inactive'}`}>
                        {queue.enabled ? '● Active' : '○ Inactive'}
                      </span>
                    </div>
                  </div>
                  
                  <div className="item-actions">
                    {editingQueue?.id === queue.id ? (
                      <button className="btn btn-small" onClick={() => setEditingQueue(null)}>Done</button>
                    ) : (
                      <button className="btn btn-small" onClick={() => setEditingQueue(queue)}>Edit</button>
                    )}
                    <button 
                      className="btn btn-small"
                      onClick={() => updateTaskQueue(queue.id, { enabled: !queue.enabled })}
                    >
                      {queue.enabled ? 'Disable' : 'Enable'}
                    </button>
                    <button 
                      className="btn btn-small btn-danger"
                      onClick={() => deleteTaskQueue(queue.id)}
                    >
                      Delete
                    </button>
                  </div>

                  {/* Tasks List */}
                  <div className="tasks-section">
                    <h4>Tasks ({queue.tasks.length})</h4>
                    
                    <div className="add-task-form">
                      <input
                        type="text"
                        placeholder="Task description"
                        value={newTaskDescription}
                        onChange={(e) => setNewTaskDescription(e.target.value)}
                        className="input input-small"
                      />
                      <input
                        type="text"
                        placeholder="Keystrokes (optional)"
                        value={newTaskKeystrokes}
                        onChange={(e) => setNewTaskKeystrokes(e.target.value)}
                        className="input input-small"
                      />
                      <input
                        type="number"
                        placeholder="Delay ms"
                        value={newTaskDelay}
                        onChange={(e) => setNewTaskDelay(e.target.value)}
                        className="input input-small"
                      />
                      <button 
                        className="btn btn-small"
                        onClick={() => addTaskToQueue(queue.id)}
                      >
                        Add
                      </button>
                    </div>

                    {queue.tasks.length === 0 ? (
                      <p className="empty">No tasks in this queue</p>
                    ) : (
                      <ul className="tasks-list">
                        {queue.tasks.map((task, idx) => (
                          <li key={idx} className="task-item">
                            <span className="task-number">{idx + 1}.</span>
                            <span className="task-desc">{task.description}</span>
                            {task.keystrokes && (
                              <span className="task-keystrokes">⌨️ {task.keystrokes}</span>
                            )}
                            {task.delay_ms && (
                              <span className="task-delay">⏱️ {task.delay_ms}ms</span>
                            )}
                            <button 
                              className="btn btn-small btn-danger"
                              onClick={() => deleteTaskFromQueue(queue.id, idx)}
                            >
                              ×
                            </button>
                          </li>
                        ))}
                      </ul>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  )
}

export default App
