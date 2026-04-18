import { useState, useEffect } from 'react'

interface HealthStatus {
  status: string
  version: string
}

function App() {
  const [health, setHealth] = useState<HealthStatus | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    fetch('/api/health')
      .then((res) => res.json())
      .then((data) => {
        setHealth(data)
        setLoading(false)
      })
      .catch((err) => {
        setError(err.message)
        setLoading(false)
      })
  }, [])

  return (
    <div className="container">
      <h1>NPC UI</h1>
      <div className="card">
        {loading ? (
          <p>Loading...</p>
        ) : error ? (
          <p className="error">Error: {error}</p>
        ) : (
          <>
            <h2>Backend Status</h2>
            <p>Status: <strong>{health?.status}</strong></p>
            <p>Version: <code>{health?.version}</code></p>
          </>
        )}
      </div>
    </div>
  )
}

export default App
