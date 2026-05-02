// Smelly: Direct state mutation in React
class Counter extends React.Component {
    increment() {
        this.state.x = this.state.x + 1;
    }
}
