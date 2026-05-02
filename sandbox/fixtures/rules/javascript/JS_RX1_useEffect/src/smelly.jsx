// Smelly: useEffect without dependency array - runs on every render
import { useEffect } from 'react';

function MyComponent() {
    useEffect(() => {
        console.log('Rendered!');
    });

    return <div>Hello</div>;
}
