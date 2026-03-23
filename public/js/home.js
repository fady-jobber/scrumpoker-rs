const nameInput = document.getElementById('name');
const createBtn = document.getElementById('createBtn');

createBtn.addEventListener('click', async () => {
    const name = nameInput.value.trim();
    if (!name) {
        alert('Please enter your name');
        return;
    }
    if (name.length > 15) {
        alert('Name must not exceed 15 characters');
        return;
    }

    const response = await fetch('/api/create_room', { method: 'POST' });
    const roomId = await response.json();

    window.location.href = `/session/${roomId}?name=${encodeURIComponent(name)}`;
});

nameInput.addEventListener('keypress', (e) => {
    if (e.key === 'Enter') createBtn.click();
});
