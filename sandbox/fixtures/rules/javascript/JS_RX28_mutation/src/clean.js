// Clean: Using setState for state updates
class Counter extends React.Component {
    increment() {
        this.setState({ x: this.state.x + 1 });
    }
}
