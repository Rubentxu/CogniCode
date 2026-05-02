// Clean: useEffect with proper dependency array
import { useEffect } from 'react';

function MyComponent() {
    const handler = () => console.log('Mounted!');
    useEffect(handler, []);

    return <div>Hello</div>;
}
