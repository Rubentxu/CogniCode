// Clean: map with key prop
import React from 'react';

function ItemList({ items }) {
    return (
        <ul>
            {items.map((i, idx) => <li key={idx}>{i}</li>)}
        </ul>
    );
}
