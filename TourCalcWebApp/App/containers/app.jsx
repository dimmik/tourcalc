import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'
import Cookies from 'js-cookie';
import LoginScreen from './login-screen.jsx';
import AuthenticatedApp from './authenticated-app.jsx'

export default class App extends React.Component {
    constructor(props) {
        super(props);
        this.state = {}
    }

    componentDidMount() {
        AppState.checkWhoAmI(this);
    }

    render() {
        if (!this.state.isAuthLoaded) {
            return (<div>Checking Who you are...</div>)
        } else {
            if (this.state.authData.type == "None") {
                return (<div><pre>{JSON.stringify(this.state.authData, null, 2)}</pre><LoginScreen app={this}/></div>)
            } else {
                return (
                    <div><pre>{JSON.stringify(this.state.authData, null, 2)}</pre><AuthenticatedApp app={this} authData={this.state.authData} /></div>
                )
            }
        }
    }
    
};

ReactDOM.render(
    <App />,
    document.getElementById("content")
);