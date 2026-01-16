const roomId = document.body.dataset.roomId;
const urlParams = new URLSearchParams(window.location.search);
let userName = urlParams.get('name');
let userId = null;
let ws = null;
let currentRoom = null;

const shareUrl = window.location.origin + '/session/' + roomId;
document.getElementById('shareUrl').textContent = shareUrl;

if (!userName) {
    document.getElementById('joinBtn').addEventListener('click', submitName);
    document.getElementById('nameInput').addEventListener('keypress', (e) => {
        if (e.key === 'Enter') submitName();
    });
} else {
    document.body.classList.add('joined');
    connectWebSocket();
}

function submitName() {
    const input = document.getElementById('nameInput');
    const name = input.value.trim();
    if (!name) {
        alert('Please enter your name');
        return;
    }

    userName = name;
    const url = new URL(window.location);
    url.searchParams.set('name', name);
    window.history.replaceState({}, '', url);

    document.body.classList.add('joined');
    connectWebSocket();
}

function copyUrl() {
    navigator.clipboard.writeText(shareUrl).then(() => {
        const btn = document.querySelector('.copy-btn');
        const originalText = btn.textContent;
        btn.textContent = 'Copied!';
        setTimeout(() => {
            btn.textContent = originalText;
        }, 2000);
    }).catch(err => {
        alert('Failed to copy URL');
    });
}

function connectWebSocket() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    ws = new WebSocket(`${protocol}//${window.location.host}/ws`);

    ws.onopen = () => {
        console.log('WebSocket connected');

        const storedUserId = localStorage.getItem(`userId_${roomId}`);

        if (storedUserId) {
            ws.send(JSON.stringify({
                type: 'Rejoin',
                room_id: roomId,
                user_id: storedUserId,
                name: userName
            }));
        } else {
            ws.send(JSON.stringify({
                type: 'Join',
                room_id: roomId,
                name: userName
            }));
        }
    };

    ws.onmessage = (event) => {
        const message = JSON.parse(event.data);
        handleServerMessage(message);
    };

    ws.onclose = () => {
        console.log('WebSocket disconnected');
        setTimeout(connectWebSocket, 3000);
    };

    ws.onerror = (error) => {
        console.error('WebSocket error:', error);
    };
}

function handleServerMessage(message) {
    if (message.type === 'Joined') {
        userId = message.user_id;
        localStorage.setItem(`userId_${roomId}`, userId);
        console.log('Joined with user ID:', userId);
    } else if (message.type === 'RoomState') {
        currentRoom = message.room;
        updateUI();
    } else if (message.type === 'Error') {
        alert('Error: ' + message.message);
    }
}

async function updateUI() {
    if (!currentRoom) return;

    const usersList = document.getElementById('usersList');
    usersList.innerHTML = '';

    const users = Object.values(currentRoom.users);

    if (users.length === 0) {
        usersList.innerHTML = '<div class="empty-state">Waiting for participants...</div>';
        return;
    }

    users.forEach(user => {
        const userCard = document.createElement('div');
        userCard.className = 'user-card';
        if (user.estimate) {
            userCard.classList.add('voted');
        }

        const nameSpan = document.createElement('span');
        nameSpan.className = 'user-name';
        if (user.id === userId) {
            nameSpan.classList.add('current-user');
            nameSpan.textContent = user.name + ' (You)';
        } else {
            nameSpan.textContent = user.name;
        }

        const estimateSpan = document.createElement('span');
        estimateSpan.className = 'estimate';

        if (user.estimate) {
            if (currentRoom.revealed) {
                estimateSpan.textContent = user.estimate;
            } else {
                estimateSpan.className = 'estimate hidden';
                estimateSpan.textContent = '✓';
            }
        } else {
            estimateSpan.className = 'estimate no-vote';
            estimateSpan.textContent = '—';
        }

        userCard.appendChild(nameSpan);
        userCard.appendChild(estimateSpan);
        usersList.appendChild(userCard);
    });

    const statsContainer = document.getElementById('stats-container');
    if (currentRoom.revealed) {
        const mean = await fetchMean();
        document.getElementById('mean-value').textContent =
            mean !== null ? mean.toFixed(1) : 'N/A';
        statsContainer.style.display = 'block';
    } else {
        statsContainer.style.display = 'none';
    }
}

function vote(estimate) {
    if (!ws || !userId) return;

    ws.send(JSON.stringify({
        type: 'Vote',
        room_id: roomId,
        user_id: userId,
        estimate: estimate
    }));
}

function showEstimates() {
    if (!ws) return;

    ws.send(JSON.stringify({
        type: 'Show',
        room_id: roomId
    }));
}

function clearEstimates() {
    if (!ws) return;

    ws.send(JSON.stringify({
        type: 'Clear',
        room_id: roomId
    }));
}

async function fetchMean() {
    const response = await fetch(`/api/room/${roomId}/mean`);
    if (!response.ok) return null;
    return await response.json();
}
