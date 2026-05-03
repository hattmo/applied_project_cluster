import { useState, useEffect } from 'react'
import './App.css'

// Types
interface MatrixUser {
  user_id: string
  display_name: string | null
}

interface AgentAssignment {
  id: string
  agent_name: string
  vm_name: string
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
  name: string
  vm_name: string
  tasks: Task[]
  enabled: boolean
  created_at: string
  updated_at: string
}

// API base URL
const API_BASE = '/api/v1'

interface AgentScaleStatus {
  replicas: number
}

function App() {
  const [activeTab, setActiveTab] = useState<'assignments' | 'queues' | 'scale'>('assignments')
  
  // Matrix agents state
  const [agents, setAgents] = useState<MatrixUser[]>([])
  const [agentsLoading, setAgentsLoading] = useState(true)
  const [agentsError, setAgentsError] = useState<string | null>(null)
  
  // Agent scaling state
  const [scaleStatus, setScaleStatus] = useState<AgentScaleStatus | null>(null)
  const [scaleLoading, setScaleLoading] = useState(false)
  const [scaleError, setScaleError] = useState<string | null>(null)
  const [desiredReplicas, setDesiredReplicas] = useState(1)
  
  // Agent Assignments state
  const [assignments, setAssignments] = useState<AgentAssignment[]>([])
  const [assignmentsLoading, setAssignmentsLoading] = useState(true)
  const [assignmentsError, setAssignmentsError] = useState<string | null>(null)
  const [showAssignmentForm, setShowAssignmentForm] = useState(false)
  const [newAssignmentAgentName, setNewAssignmentAgentName] = useState('')
  const [newAssignmentVmName, setNewAssignmentVmName] = useState('')
  const [editingAssignment, setEditingAssignment] = useState<AgentAssignment | null>(null)
  
  // Available VMs from vmware_gateway
  const [availableVms, setAvailableVms] = useState<string[]>([])
  const [availableVmsLoading, setAvailableVmsLoading] = useState(true)

  // Task Queues state
  const [taskQueues, setTaskQueues] = useState<TaskQueue[]>([])
  const [queueLoading, setQueueLoading] = useState(true)
  const [queueError, setQueueError] = useState<string | null>(null)
  const [showQueueForm, setShowQueueForm] = useState(false)
  const [newQueueName, setNewQueueName] = useState('')
  const [newQueueVmName, setNewQueueVmName] = useState('')
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

  // Load Agent Assignments
  useEffect(() => {
    fetchAssignments()
  }, [])

  // Load Task Queues when switching to queues tab
  useEffect(() => {
    if (activeTab === 'queues') {
      fetchTaskQueues()
    }
  }, [activeTab])

  // Load scale status when switching to scale tab
  useEffect(() => {
    if (activeTab === 'scale') {
      fetchScaleStatus()
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

  async function fetchAssignments() {
    try {
      const res = await fetch(`${API_BASE}/agent-assignments`)
      if (!res.ok) throw new Error('Failed to fetch agent assignments')
      const data = await res.json()
      setAssignments(data)
      setAssignmentsLoading(false)
    } catch (err) {
      setAssignmentsError(err instanceof Error ? err.message : 'Unknown error')
      setAssignmentsLoading(false)
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

  async function fetchScaleStatus() {
    try {
      const res = await fetch(`${API_BASE}/agents/scale`)
      if (!res.ok) throw new Error('Failed to fetch scale status')
      const data = await res.json()
      setScaleStatus(data)
      setDesiredReplicas(data.replicas)
      setScaleLoading(false)
    } catch (err) {
      setScaleError(err instanceof Error ? err.message : 'Unknown error')
      setScaleLoading(false)
    }
  }

  async function scaleAgents(replicas: number) {
    setScaleLoading(true)
    try {
      const res = await fetch(`${API_BASE}/agents/scale`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ replicas }),
      })
      if (!res.ok) throw new Error('Failed to scale agents')
      const data = await res.json()
      setScaleStatus(data)
      setScaleLoading(false)
    } catch (err) {
      setScaleError(err instanceof Error ? err.message : 'Unknown error')
      setScaleLoading(false)
    }
  }

  async function createAssignment() {
    try {
      const res = await fetch(`${API_BASE}/agent-assignments`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ agent_name: newAssignmentAgentName, vm_name: newAssignmentVmName, enabled: true }),
      })
      if (!res.ok) throw new Error('Failed to create assignment')
      await fetchAssignments()
      setNewAssignmentAgentName('')
      setNewAssignmentVmName('')
      setShowAssignmentForm(false)
    } catch (err) {
      setAssignmentsError(err instanceof Error ? err.message : 'Unknown error')
    }
  }

  async function updateAssignment(id: string, updates: Partial<AgentAssignment>) {
    try {
      const res = await fetch(`${API_BASE}/agent-assignments/${id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(updates),
      })
      if (!res.ok) throw new Error('Failed to update assignment')
      await fetchAssignments()
      setEditingAssignment(null)
    } catch (err) {
      setAssignmentsError(err instanceof Error ? err.message : 'Unknown error')
    }
  }

  async function deleteAssignment(id: string) {
    if (!confirm('Delete this assignment?')) return
    try {
      const res = await fetch(`${API_BASE}/agent-assignments/${id}`, { method: 'DELETE' })
      if (!res.ok) throw new Error('Failed to delete assignment')
      await fetchAssignments()
    } catch (err) {
      setAssignmentsError(err instanceof Error ? err.message : 'Unknown error')
    }
  }

  async function createTaskQueue() {
    try {
      const res = await fetch(`${API_BASE}/task-queues`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ 
          vm_name: newQueueVmName, 
          name: newQueueName, 
          tasks: [],
          enabled: true
        }),
      })
      if (!res.ok) throw new Error('Failed to create task queue')
      await fetchTaskQueues()
      setNewQueueName('')
      setNewQueueVmName('')
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
          className={`tab ${activeTab === 'assignments' ? 'active' : ''}`}
          onClick={() => setActiveTab('assignments')}
        >
          Agent Assignments
        </button>
        <button 
          className={`tab ${activeTab === 'queues' ? 'active' : ''}`}
          onClick={() => setActiveTab('queues')}
        >
          Task Queues
        </button>
        <button 
          className={`tab ${activeTab === 'scale' ? 'active' : ''}`}
          onClick={() => setActiveTab('scale')}
        >
          Scale Agents
        </button>
      </div>

      {/* Agent Assignments Tab */}
      {activeTab === 'assignments' && (
        <div className="card">
          <div className="card-header">
            <h2>Agent Assignments</h2>
            <button className="btn btn-primary" onClick={() => setShowAssignmentForm(!showAssignmentForm)}>
              {showAssignmentForm ? 'Cancel' : '+ Add Assignment'}
            </button>
          </div>

          {showAssignmentForm && (
            <div className="form">
              <select
                value={newAssignmentAgentName}
                onChange={(e) => setNewAssignmentAgentName(e.target.value)}
                className="input"
              >
                <option value="">Select Agent</option>
                {agentsLoading ? (
                  <option disabled>Loading agents...</option>
                ) : agentsError ? (
                  <option disabled>Error loading agents</option>
                ) : agents.length === 0 ? (
                  <option disabled>No agents available</option>
                ) : (
                  agents.map((agent) => (
                    <option key={agent.user_id} value={agent.user_id}>
                      {agent.display_name || agent.user_id}
                    </option>
                  ))
                )}
              </select>
              <select
                value={newAssignmentVmName}
                onChange={(e) => setNewAssignmentVmName(e.target.value)}
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
              <button className="btn btn-primary" onClick={createAssignment}>Create</button>
            </div>
          )}

          {assignmentsLoading ? (
            <p>Loading...</p>
          ) : assignmentsError ? (
            <p className="error">Error: {assignmentsError}</p>
          ) : assignments.length === 0 ? (
            <p className="empty">No assignments yet. Add one to get started!</p>
          ) : (
            <div className="list">
              {assignments.map((assignment) => (
                <div key={assignment.id} className="list-item">
                  {editingAssignment?.id === assignment.id ? (
                    <div className="edit-form">
                      <select
                        defaultValue={assignment.agent_name}
                        onBlur={(e) => updateAssignment(assignment.id, { agent_name: e.target.value })}
                        className="input"
                      >
                        {agents.map((agent) => (
                          <option key={agent.user_id} value={agent.user_id}>
                            {agent.display_name || agent.user_id}
                          </option>
                        ))}
                      </select>
                      <select
                        defaultValue={assignment.vm_name}
                        onBlur={(e) => updateAssignment(assignment.id, { vm_name: e.target.value })}
                        className="input"
                      >
                        {availableVms.map((vm) => (
                          <option key={vm} value={vm}>{vm}</option>
                        ))}
                      </select>
                    </div>
                  ) : (
                    <div className="item-content">
                      <div>
                        <strong>{assignment.vm_name}</strong>
                        <span className="badge">Agent: {getAgentDisplayName(assignment.agent_name)}</span>
                      </div>
                      <span className={`status ${assignment.enabled ? 'active' : 'inactive'}`}>
                        {assignment.enabled ? '● Active' : '○ Inactive'}
                      </span>
                    </div>
                  )}
                  <div className="item-actions">
                    {editingAssignment?.id === assignment.id ? (
                      <button className="btn btn-small" onClick={() => setEditingAssignment(null)}>Done</button>
                    ) : (
                      <button className="btn btn-small" onClick={() => setEditingAssignment(assignment)}>Edit</button>
                    )}
                    <button 
                      className="btn btn-small"
                      onClick={() => updateAssignment(assignment.id, { enabled: !assignment.enabled })}
                    >
                      {assignment.enabled ? 'Disable' : 'Enable'}
                    </button>
                    <button 
                      className="btn btn-small btn-danger"
                      onClick={() => deleteAssignment(assignment.id)}
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
                value={newQueueVmName}
                onChange={(e) => setNewQueueVmName(e.target.value)}
                className="input"
              >
                <option value="">Select VM</option>
                {assignments.map((a) => (
                  <option key={a.id} value={a.vm_name}>{a.vm_name}</option>
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
                      <span className="badge">VM: {queue.vm_name}</span>
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

      {/* Scale Agents Tab */}
      {activeTab === 'scale' && (
        <div className="card">
          <div className="card-header">
            <h2>Scale Agents</h2>
          </div>

          {scaleLoading ? (
            <p>Loading...</p>
          ) : scaleError ? (
            <p className="error">Error: {scaleError}</p>
          ) : scaleStatus ? (
            <div className="scale-container">
              <div className="scale-info">
                <p>Current replicas: <strong>{scaleStatus.replicas}</strong></p>
              </div>

              <div className="scale-controls">
                <label>Number of agents (1-5):</label>
                <input
                  type="range"
                  min="1"
                  max="5"
                  value={desiredReplicas}
                  onChange={(e) => setDesiredReplicas(parseInt(e.target.value))}
                  className="scale-slider"
                />
                <div className="scale-values">
                  <span>1</span>
                  <span>2</span>
                  <span>3</span>
                  <span>4</span>
                  <span>5</span>
                </div>
                <button 
                  className="btn btn-primary"
                  onClick={() => scaleAgents(desiredReplicas)}
                  disabled={scaleLoading}
                >
                  {scaleLoading ? 'Scaling...' : 'Apply'}
                </button>
              </div>
            </div>
          ) : (
            <p className="empty">Unable to load scale status</p>
          )}
        </div>
      )}
    </div>
  )
}

export default App
